use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use tracing::info;

use crate::config::settings::Settings;
use crate::hid::hid_manager::{enumerate_devices, open_matching_devices, HidDeviceFilter};
use crate::hid::inzone_hub::{
    build_hub_request, flush_hub_input, hub_device_filter, parse_hub_battery_report,
    parse_hub_battery_report_for_sequence, read_hub_reports_for, send_hub_request_with_method,
};

const REPORT_SIZE: usize = 64;

pub enum Command {
    Tray,
    ListHid(HidListArgs),
    DumpHid(HidDumpArgs),
    CompareDumps(CompareDumpsArgs),
    CaptureState(CaptureStateArgs),
    CompareStateDirs(CompareStateDirsArgs),
    CaptureFeatureSeries(CaptureFeatureSeriesArgs),
    AnalyzeFeatureSeries(AnalyzeFeatureSeriesArgs),
    QueryHubBattery(QueryHubBatteryArgs),
}

#[derive(Debug, Clone)]
pub struct HidListArgs {
    pub filter: HidDeviceFilter,
}

#[derive(Debug, Clone)]
pub struct HidDumpArgs {
    pub filter: HidDeviceFilter,
    pub count: usize,
    pub timeout_ms: i32,
    pub output_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CompareDumpsArgs {
    pub left: PathBuf,
    pub right: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CaptureStateArgs {
    pub label: String,
    pub count: usize,
    pub timeout_ms: i32,
    pub all_collections: bool,
}

#[derive(Debug, Clone)]
pub struct CompareStateDirsArgs {
    pub left_dir: PathBuf,
    pub right_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CaptureFeatureSeriesArgs {
    pub label: String,
    pub samples: usize,
    pub interval_ms: u64,
    pub all_collections: bool,
}

#[derive(Debug, Clone)]
pub struct AnalyzeFeatureSeriesArgs {
    pub series_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct QueryHubBatteryArgs {
    pub count: usize,
    pub timeout_ms: i32,
    pub interval_ms: u64,
}

impl Command {
    pub fn parse_from_env() -> Result<Self> {
        let mut args = env::args().skip(1);
        let Some(command) = args.next() else {
            return Ok(Self::Tray);
        };

        match command.as_str() {
            "list-hid" => Ok(Self::ListHid(HidListArgs {
                filter: parse_filter_args(args.collect())?,
            })),
            "dump-hid" => parse_dump_hid(args.collect()),
            "compare-dumps" => parse_compare_dumps(args.collect()),
            "capture-state" => parse_capture_state(args.collect()),
            "compare-state-dirs" => parse_compare_state_dirs(args.collect()),
            "capture-feature-series" => parse_capture_feature_series(args.collect()),
            "analyze-feature-series" => parse_analyze_feature_series(args.collect()),
            "query-hub-battery" => parse_query_hub_battery(args.collect()),
            other => bail!("unknown command: {other}"),
        }
    }
}

pub fn run_list_hid(args: HidListArgs) -> Result<()> {
    let devices = enumerate_devices(&args.filter)?;
    if devices.is_empty() {
        println!("no matching hid devices found");
        return Ok(());
    }

    for device in devices {
        println!(
            "VID={:04X} PID={:04X} IF={} USAGE_PAGE={:04X} USAGE={:04X}",
            device.vendor_id,
            device.product_id,
            device.interface_number,
            device.usage_page,
            device.usage
        );
        println!("  path={}", device.path);
        println!(
            "  manufacturer={}",
            device.manufacturer_string.as_deref().unwrap_or("-")
        );
        println!(
            "  product={}",
            device.product_string.as_deref().unwrap_or("-")
        );
        println!(
            "  serial={}",
            device.serial_number.as_deref().unwrap_or("-")
        );
    }

    Ok(())
}

pub fn run_dump_hid(args: HidDumpArgs) -> Result<()> {
    fs::create_dir_all(&args.output_dir)?;
    let devices = open_matching_devices(&args.filter)?;
    if devices.is_empty() {
        println!("no matching hid devices found");
        return Ok(());
    }

    for (index, (snapshot, device)) in devices.into_iter().enumerate() {
        println!(
            "dumping device #{index}: VID={:04X} PID={:04X} IF={} USAGE_PAGE={:04X} USAGE={:04X}",
            snapshot.vendor_id,
            snapshot.product_id,
            snapshot.interface_number,
            snapshot.usage_page,
            snapshot.usage
        );
        println!("  path={}", snapshot.path);

        let prefix = format!(
            "vid{:04x}_pid{:04x}_if{}_up{:04x}_u{:04x}",
            snapshot.vendor_id,
            snapshot.product_id,
            snapshot.interface_number,
            snapshot.usage_page,
            snapshot.usage
        );

        dump_report_descriptor(&args.output_dir, &prefix, &device)?;
        probe_feature_reports(&args.output_dir, &prefix, &device)?;

        for sample_index in 0..args.count {
            let mut buffer = vec![0_u8; REPORT_SIZE];
            match device.read_timeout(&mut buffer, args.timeout_ms) {
                Ok(0) => {
                    println!("  sample {sample_index}: timeout");
                }
                Ok(size) => {
                    buffer.truncate(size);
                    let path = write_dump_file(&args.output_dir, &prefix, sample_index, &buffer)?;
                    println!(
                        "  sample {sample_index}: {} bytes -> {}",
                        size,
                        path.display()
                    );
                    println!("    hex={}", to_hex(&buffer));
                }
                Err(error) => {
                    println!("  sample {sample_index}: read failed: {error}");
                    break;
                }
            }

            std::thread::sleep(Duration::from_millis(250));
        }
    }

    Ok(())
}

pub fn run_compare_dumps(args: CompareDumpsArgs) -> Result<()> {
    let left =
        fs::read(&args.left).with_context(|| format!("failed to read {}", args.left.display()))?;
    let right = fs::read(&args.right)
        .with_context(|| format!("failed to read {}", args.right.display()))?;

    println!("left:  {}", args.left.display());
    println!("right: {}", args.right.display());
    println!("left size={} right size={}", left.len(), right.len());

    let max_len = left.len().max(right.len());
    let mut difference_count = 0_usize;

    for index in 0..max_len {
        let left_byte = left.get(index).copied();
        let right_byte = right.get(index).copied();
        if left_byte == right_byte {
            continue;
        }

        difference_count += 1;
        println!(
            "offset 0x{index:04X}: left={} right={}",
            format_optional_byte(left_byte),
            format_optional_byte(right_byte)
        );
    }

    if difference_count == 0 {
        println!("no byte differences");
    } else {
        println!("differences: {difference_count}");
    }

    Ok(())
}

pub fn run_compare_text_files(left: &Path, right: &Path) -> Result<()> {
    let left_text =
        fs::read_to_string(left).with_context(|| format!("failed to read {}", left.display()))?;
    let right_text =
        fs::read_to_string(right).with_context(|| format!("failed to read {}", right.display()))?;

    println!("left:  {}", left.display());
    println!("right: {}", right.display());

    if left_text == right_text {
        println!("no text differences");
        return Ok(());
    }

    println!("text differs");
    println!("  left : {}", left_text.replace('\n', "\\n"));
    println!("  right: {}", right_text.replace('\n', "\\n"));
    Ok(())
}

pub fn run_capture_state(args: CaptureStateArgs) -> Result<()> {
    let settings = Settings::load()?;
    let output_dir = create_state_dir(&args.label)?;
    fs::create_dir_all(&output_dir)?;

    let filter = if args.all_collections {
        HidDeviceFilter {
            vendor_id: Some(settings.vendor_id),
            product_id: settings.product_id,
            interface_number: None,
            usage_page: None,
            usage: None,
        }
    } else {
        app_device_filter(&settings)
    };

    let devices = open_matching_devices(&filter)?;
    if devices.is_empty() {
        println!("no matching hid devices found");
        return Ok(());
    }

    let multiple_devices = devices.len() > 1;

    for (index, (snapshot, device)) in devices.into_iter().enumerate() {
        let device_dir = if multiple_devices {
            output_dir.join(format!(
                "device_{index}_if{}_up{:04x}_u{:04x}",
                snapshot.interface_number, snapshot.usage_page, snapshot.usage
            ))
        } else {
            output_dir.clone()
        };
        fs::create_dir_all(&device_dir)?;

        fs::write(
            device_dir.join("device_info.txt"),
            format!(
                "VID={:04X}\nPID={:04X}\nIF={}\nUSAGE_PAGE={:04X}\nUSAGE={:04X}\nPATH={}\n",
                snapshot.vendor_id,
                snapshot.product_id,
                snapshot.interface_number,
                snapshot.usage_page,
                snapshot.usage,
                snapshot.path
            ),
        )?;

        println!(
            "capturing state '{}' into {}",
            args.label,
            device_dir.display()
        );

        let mut descriptor = vec![0_u8; hidapi::MAX_REPORT_DESCRIPTOR_SIZE];
        let descriptor_size = device
            .get_report_descriptor(&mut descriptor)
            .context("failed to read report descriptor")?;
        descriptor.truncate(descriptor_size);
        fs::write(device_dir.join("descriptor.bin"), &descriptor)?;

        let report_ids = if !args.all_collections && !settings.feature_report_ids.is_empty() {
            settings.feature_report_ids.clone()
        } else if settings.feature_report_ids.is_empty() {
            extract_report_ids(&descriptor)
        } else {
            let mut ids = extract_report_ids(&descriptor);
            for id in &settings.feature_report_ids {
                if !ids.contains(id) {
                    ids.push(*id);
                }
            }
            ids
        };

        for report_id in report_ids {
            let mut feature = vec![0_u8; settings.feature_report_size];
            feature[0] = report_id;
            match device.get_feature_report(&mut feature) {
                Ok(size) if size > 0 => {
                    feature.truncate(size);
                    fs::write(
                        device_dir.join(format!("feature_{report_id:02x}.bin")),
                        &feature,
                    )?;
                }
                Ok(_) => {}
                Err(error) => {
                    fs::write(
                        device_dir.join(format!("feature_{report_id:02x}.txt")),
                        error.to_string(),
                    )?;
                }
            }
        }

        for sample_index in 0..args.count {
            let mut buffer = vec![0_u8; REPORT_SIZE];
            match device.read_timeout(&mut buffer, args.timeout_ms) {
                Ok(size) if size > 0 => {
                    buffer.truncate(size);
                    fs::write(
                        device_dir.join(format!("input_{sample_index:03}.bin")),
                        &buffer,
                    )?;
                }
                Ok(_) => {
                    fs::write(
                        device_dir.join(format!("input_{sample_index:03}.txt")),
                        "timeout",
                    )?;
                }
                Err(error) => {
                    fs::write(
                        device_dir.join(format!("input_{sample_index:03}.txt")),
                        error.to_string(),
                    )?;
                    break;
                }
            }
        }
    }

    println!("state capture saved to {}", output_dir.display());
    Ok(())
}

pub fn run_compare_state_dirs(args: CompareStateDirsArgs) -> Result<()> {
    let left_files = list_state_files(&args.left_dir)?;
    let right_files = list_state_files(&args.right_dir)?;

    println!("left dir:  {}", args.left_dir.display());
    println!("right dir: {}", args.right_dir.display());

    for relative_path in left_files.keys() {
        if let Some(right_path) = right_files.get(relative_path) {
            match relative_path.extension().and_then(|ext| ext.to_str()) {
                Some(ext) if ext.eq_ignore_ascii_case("bin") => {
                    run_compare_dumps(CompareDumpsArgs {
                        left: left_files[relative_path].clone(),
                        right: right_path.clone(),
                    })?;
                }
                Some(ext) if ext.eq_ignore_ascii_case("txt") => {
                    run_compare_text_files(&left_files[relative_path], right_path)?;
                }
                _ => {}
            }
        } else {
            println!("missing on right: {}", relative_path.display());
        }
    }

    for relative_path in right_files.keys() {
        if !left_files.contains_key(relative_path) {
            println!("missing on left: {}", relative_path.display());
        }
    }

    Ok(())
}

pub fn run_capture_feature_series(args: CaptureFeatureSeriesArgs) -> Result<()> {
    let settings = Settings::load()?;
    let output_dir = create_series_dir(&args.label)?;
    fs::create_dir_all(&output_dir)?;
    fs::write(
        output_dir.join("series_info.txt"),
        format!(
            "label={}\nsamples={}\ninterval_ms={}\nall_collections={}\n",
            args.label, args.samples, args.interval_ms, args.all_collections
        ),
    )?;

    let filter = if args.all_collections {
        HidDeviceFilter {
            vendor_id: Some(settings.vendor_id),
            product_id: settings.product_id,
            interface_number: None,
            usage_page: None,
            usage: None,
        }
    } else {
        app_device_filter(&settings)
    };

    let devices = open_matching_devices(&filter)?;
    if devices.is_empty() {
        println!("no matching hid devices found");
        return Ok(());
    }

    let multiple_devices = devices.len() > 1;
    for (index, (snapshot, device)) in devices.into_iter().enumerate() {
        let device_dir = if multiple_devices {
            output_dir.join(format!(
                "device_{index}_if{}_up{:04x}_u{:04x}",
                snapshot.interface_number, snapshot.usage_page, snapshot.usage
            ))
        } else {
            output_dir.clone()
        };
        fs::create_dir_all(&device_dir)?;

        fs::write(
            device_dir.join("device_info.txt"),
            format!(
                "VID={:04X}\nPID={:04X}\nIF={}\nUSAGE_PAGE={:04X}\nUSAGE={:04X}\nPATH={}\n",
                snapshot.vendor_id,
                snapshot.product_id,
                snapshot.interface_number,
                snapshot.usage_page,
                snapshot.usage,
                snapshot.path
            ),
        )?;

        let mut descriptor = vec![0_u8; hidapi::MAX_REPORT_DESCRIPTOR_SIZE];
        let descriptor_size = device
            .get_report_descriptor(&mut descriptor)
            .context("failed to read report descriptor")?;
        descriptor.truncate(descriptor_size);
        fs::write(device_dir.join("descriptor.bin"), &descriptor)?;

        let report_ids =
            feature_report_ids_for_descriptor(&settings, args.all_collections, &descriptor);
        println!(
            "capturing feature series '{}' into {} reports={}",
            args.label,
            device_dir.display(),
            report_ids
                .iter()
                .map(|id| format!("{id:02X}"))
                .collect::<Vec<_>>()
                .join(",")
        );

        let mut index_log = String::from("sample,timestamp_ms,report_id,status,file\n");
        for sample_index in 0..args.samples {
            let timestamp_ms = current_timestamp_ms()?;
            for &report_id in &report_ids {
                let mut feature = vec![0_u8; settings.feature_report_size];
                feature[0] = report_id;
                match device.get_feature_report(&mut feature) {
                    Ok(size) if size > 0 => {
                        feature.truncate(size);
                        let file_name = format!("feature_{report_id:02x}_{sample_index:03}.bin");
                        fs::write(device_dir.join(&file_name), &feature)?;
                        index_log.push_str(&format!(
                            "{sample_index},{timestamp_ms},{report_id:02X},ok,{file_name}\n"
                        ));
                    }
                    Ok(_) => {
                        let file_name = format!("feature_{report_id:02x}_{sample_index:03}.txt");
                        fs::write(device_dir.join(&file_name), "empty")?;
                        index_log.push_str(&format!(
                            "{sample_index},{timestamp_ms},{report_id:02X},empty,{file_name}\n"
                        ));
                    }
                    Err(error) => {
                        let file_name = format!("feature_{report_id:02x}_{sample_index:03}.txt");
                        fs::write(device_dir.join(&file_name), error.to_string())?;
                        index_log.push_str(&format!(
                            "{sample_index},{timestamp_ms},{report_id:02X},error,{file_name}\n"
                        ));
                    }
                }
            }

            if sample_index + 1 < args.samples {
                std::thread::sleep(Duration::from_millis(args.interval_ms));
            }
        }

        fs::write(device_dir.join("series_index.csv"), index_log)?;
    }

    println!("feature series saved to {}", output_dir.display());
    Ok(())
}

pub fn run_analyze_feature_series(args: AnalyzeFeatureSeriesArgs) -> Result<()> {
    let groups = collect_feature_series_groups(&args.series_dir)?;
    if groups.is_empty() {
        println!(
            "no feature series .bin files found under {}",
            args.series_dir.display()
        );
        return Ok(());
    }

    println!("series dir: {}", args.series_dir.display());
    for ((device_key, report_id), mut samples) in groups {
        samples.sort_by_key(|sample| sample.sample_index);
        println!(
            "\n{} report {:02X}: {} samples",
            device_key,
            report_id,
            samples.len()
        );

        if samples.len() < 2 {
            println!("  not enough samples to compare");
            continue;
        }

        let max_len = samples
            .iter()
            .map(|sample| sample.data.len())
            .max()
            .unwrap_or(0);
        let mut change_counts = vec![0_usize; max_len];
        let mut distinct_values: Vec<std::collections::BTreeSet<u8>> = (0..max_len)
            .map(|_| std::collections::BTreeSet::new())
            .collect();

        for sample in &samples {
            for (offset, byte) in sample.data.iter().copied().enumerate() {
                distinct_values[offset].insert(byte);
            }
        }

        for pair in samples.windows(2) {
            let left = &pair[0].data;
            let right = &pair[1].data;
            let len = left.len().max(right.len());
            for (offset, count) in change_counts.iter_mut().enumerate().take(len) {
                if left.get(offset) != right.get(offset) {
                    *count += 1;
                }
            }
        }

        let changed_offsets = change_counts
            .iter()
            .enumerate()
            .filter_map(|(offset, count)| (*count > 0).then_some(offset))
            .collect::<Vec<_>>();

        if changed_offsets.is_empty() {
            println!("  no sample-to-sample byte changes");
            continue;
        }

        println!("  changed offsets: {}", changed_offsets.len());
        println!(
            "  changed ranges: {}",
            format_offset_ranges(&changed_offsets)
        );

        for offset in changed_offsets.iter().take(80) {
            let values = distinct_values[*offset]
                .iter()
                .map(|value| format!("{value:02X}"))
                .collect::<Vec<_>>()
                .join(",");
            println!(
                "  offset 0x{offset:04X}: changes={} distinct={} values={}",
                change_counts[*offset],
                distinct_values[*offset].len(),
                values
            );
        }

        if changed_offsets.len() > 80 {
            println!(
                "  ... {} more changed offsets omitted",
                changed_offsets.len() - 80
            );
        }

        println!("  4-byte buckets:");
        for bucket in changed_offsets
            .iter()
            .map(|offset| offset / 4 * 4)
            .collect::<std::collections::BTreeSet<_>>()
        {
            let count = changed_offsets
                .iter()
                .filter(|offset| (**offset / 4 * 4) == bucket)
                .count();
            println!(
                "    0x{bucket:04X}-0x{:04X}: {} changed byte(s)",
                bucket + 3,
                count
            );
        }
    }

    Ok(())
}

pub fn run_query_hub_battery(args: QueryHubBatteryArgs) -> Result<()> {
    let settings = Settings::load()?;
    let mut hid = crate::hid::hid_manager::HidManager::new(hub_device_filter(&settings))?;

    println!(
        "querying INZONE Hub-compatible HID collection: VID={:04X} PID={} IF={:?} USAGE_PAGE=FF04 USAGE=0001",
        settings.vendor_id,
        settings
            .product_id
            .map(|product_id| format!("{product_id:04X}"))
            .unwrap_or_else(|| "-".to_string()),
        settings.interface_number
    );

    let mut sequence = 1_u8;
    for sample_index in 0..args.count {
        let request = build_hub_request(0x04, sequence);
        println!(
            "sample {sample_index}: request seq={sequence} hex={}",
            to_hex(&request)
        );

        flush_hub_input(&mut hid);
        match send_hub_request_with_method(&mut hid, 0x04, sequence)? {
            Some(method) => println!("  sent via {method:?}"),
            None => {
                println!("  no matching hub HID collection");
                sequence = next_sequence(sequence);
                continue;
            }
        }

        {
            let reports = read_hub_reports_for(&mut hid, args.timeout_ms)?;
            if reports.is_empty() {
                println!("  no hub response");
            }

            for (report_index, report) in reports.iter().enumerate() {
                println!("  response {report_index}: hex={}", to_hex(report));
                let Some(parsed) = parse_hub_battery_report(report) else {
                    println!("    parsed: non-battery report");
                    continue;
                };
                let sequence_label =
                    if parse_hub_battery_report_for_sequence(report, sequence).is_some() {
                        "explicit-response"
                    } else {
                        "async-or-stale"
                    };
                println!(
                    "    parsed {sequence_label} left={} right={} case={}",
                    format_optional_percent(parsed.left),
                    format_optional_percent(parsed.right),
                    format_optional_percent(parsed.case)
                );
            }
        }

        sequence = next_sequence(sequence);
        if sample_index + 1 < args.count {
            std::thread::sleep(Duration::from_millis(args.interval_ms));
        }
    }

    Ok(())
}

pub fn app_device_filter(settings: &Settings) -> HidDeviceFilter {
    HidDeviceFilter {
        vendor_id: Some(settings.vendor_id),
        product_id: settings.product_id,
        interface_number: settings.interface_number,
        usage_page: settings.usage_page,
        usage: settings.usage,
    }
}

fn parse_dump_hid(args: Vec<String>) -> Result<Command> {
    let mut count = 5_usize;
    let mut timeout_ms = 1000_i32;
    let mut output_dir = PathBuf::from("dumps");
    let mut filter_tokens = Vec::new();

    let mut iter = args.into_iter();
    while let Some(token) = iter.next() {
        match token.as_str() {
            "--count" => {
                count = iter
                    .next()
                    .context("missing value for --count")?
                    .parse()
                    .context("invalid --count value")?;
            }
            "--timeout" => {
                timeout_ms = iter
                    .next()
                    .context("missing value for --timeout")?
                    .parse()
                    .context("invalid --timeout value")?;
            }
            "--output-dir" => {
                output_dir = PathBuf::from(iter.next().context("missing value for --output-dir")?);
            }
            other => filter_tokens.push(other.to_string()),
        }
    }

    Ok(Command::DumpHid(HidDumpArgs {
        filter: parse_filter_args(filter_tokens)?,
        count,
        timeout_ms,
        output_dir,
    }))
}

fn parse_compare_dumps(args: Vec<String>) -> Result<Command> {
    if args.len() != 2 {
        bail!("compare-dumps expects exactly two file paths");
    }

    Ok(Command::CompareDumps(CompareDumpsArgs {
        left: PathBuf::from(&args[0]),
        right: PathBuf::from(&args[1]),
    }))
}

fn parse_capture_state(args: Vec<String>) -> Result<Command> {
    let mut count = 1_usize;
    let mut timeout_ms = 1000_i32;
    let mut all_collections = false;
    let mut label = None;
    let mut iter = args.into_iter();

    while let Some(token) = iter.next() {
        match token.as_str() {
            "--count" => {
                count = iter
                    .next()
                    .context("missing value for --count")?
                    .parse()
                    .context("invalid --count value")?;
            }
            "--timeout" => {
                timeout_ms = iter
                    .next()
                    .context("missing value for --timeout")?
                    .parse()
                    .context("invalid --timeout value")?;
            }
            "--all-collections" => {
                all_collections = true;
            }
            other if label.is_none() => {
                label = Some(other.to_string());
            }
            other => bail!("unknown option: {other}"),
        }
    }

    Ok(Command::CaptureState(CaptureStateArgs {
        label: label.context("capture-state requires a label")?,
        count,
        timeout_ms,
        all_collections,
    }))
}

fn parse_compare_state_dirs(args: Vec<String>) -> Result<Command> {
    if args.len() != 2 {
        bail!("compare-state-dirs expects exactly two directories");
    }

    Ok(Command::CompareStateDirs(CompareStateDirsArgs {
        left_dir: PathBuf::from(&args[0]),
        right_dir: PathBuf::from(&args[1]),
    }))
}

fn parse_capture_feature_series(args: Vec<String>) -> Result<Command> {
    let mut samples = 80_usize;
    let mut interval_ms = 250_u64;
    let mut all_collections = false;
    let mut label = None;
    let mut iter = args.into_iter();

    while let Some(token) = iter.next() {
        match token.as_str() {
            "--samples" => {
                samples = iter
                    .next()
                    .context("missing value for --samples")?
                    .parse()
                    .context("invalid --samples value")?;
            }
            "--interval-ms" => {
                interval_ms = iter
                    .next()
                    .context("missing value for --interval-ms")?
                    .parse()
                    .context("invalid --interval-ms value")?;
            }
            "--all-collections" => {
                all_collections = true;
            }
            other if label.is_none() => {
                label = Some(other.to_string());
            }
            other => bail!("unknown option: {other}"),
        }
    }

    Ok(Command::CaptureFeatureSeries(CaptureFeatureSeriesArgs {
        label: label.context("capture-feature-series requires a label")?,
        samples,
        interval_ms,
        all_collections,
    }))
}

fn parse_analyze_feature_series(args: Vec<String>) -> Result<Command> {
    if args.len() != 1 {
        bail!("analyze-feature-series expects exactly one series directory");
    }

    Ok(Command::AnalyzeFeatureSeries(AnalyzeFeatureSeriesArgs {
        series_dir: PathBuf::from(&args[0]),
    }))
}

fn parse_query_hub_battery(args: Vec<String>) -> Result<Command> {
    let mut count = 1_usize;
    let mut timeout_ms = 1000_i32;
    let mut interval_ms = 1000_u64;
    let mut iter = args.into_iter();

    while let Some(token) = iter.next() {
        match token.as_str() {
            "--count" => {
                count = iter
                    .next()
                    .context("missing value for --count")?
                    .parse()
                    .context("invalid --count value")?;
            }
            "--timeout" => {
                timeout_ms = iter
                    .next()
                    .context("missing value for --timeout")?
                    .parse()
                    .context("invalid --timeout value")?;
            }
            "--interval-ms" => {
                interval_ms = iter
                    .next()
                    .context("missing value for --interval-ms")?
                    .parse()
                    .context("invalid --interval-ms value")?;
            }
            other => bail!("unknown option: {other}"),
        }
    }

    Ok(Command::QueryHubBattery(QueryHubBatteryArgs {
        count,
        timeout_ms,
        interval_ms,
    }))
}

fn parse_filter_args(args: Vec<String>) -> Result<HidDeviceFilter> {
    let mut filter = HidDeviceFilter::default();
    let mut iter = args.into_iter();

    while let Some(token) = iter.next() {
        match token.as_str() {
            "--vendor" => {
                filter.vendor_id = Some(parse_u16_hex(
                    &iter.next().context("missing value for --vendor")?,
                )?);
            }
            "--product" => {
                filter.product_id = Some(parse_u16_hex(
                    &iter.next().context("missing value for --product")?,
                )?);
            }
            "--interface" => {
                filter.interface_number = Some(
                    iter.next()
                        .context("missing value for --interface")?
                        .parse()
                        .context("invalid --interface value")?,
                );
            }
            "--usage-page" => {
                filter.usage_page = Some(parse_u16_hex(
                    &iter.next().context("missing value for --usage-page")?,
                )?);
            }
            "--usage" => {
                filter.usage = Some(parse_u16_hex(
                    &iter.next().context("missing value for --usage")?,
                )?);
            }
            "--inzone" => {
                let settings = Settings::load()?;
                filter = app_device_filter(&settings);
            }
            other => bail!("unknown option: {other}"),
        }
    }

    Ok(filter)
}

fn parse_u16_hex(value: &str) -> Result<u16> {
    let trimmed = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    u16::from_str_radix(trimmed, 16).with_context(|| format!("invalid hex value: {value}"))
}

fn write_dump_file(
    output_dir: &Path,
    prefix: &str,
    sample_index: usize,
    data: &[u8],
) -> Result<PathBuf> {
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before unix epoch")?
        .as_millis();
    let path = output_dir.join(format!("{prefix}_{timestamp_ms}_{sample_index:03}.bin"));
    fs::write(&path, data)?;
    info!("wrote hid dump {}", path.display());
    Ok(path)
}

fn to_hex(data: &[u8]) -> String {
    data.iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_optional_byte(byte: Option<u8>) -> String {
    match byte {
        Some(byte) => format!("0x{byte:02X} ({byte})"),
        None => "-".to_string(),
    }
}

fn format_optional_percent(value: Option<u8>) -> String {
    value
        .map(|percent| format!("{percent}%"))
        .unwrap_or_else(|| "unavailable".to_string())
}

fn next_sequence(sequence: u8) -> u8 {
    match sequence.wrapping_add(1) {
        0 => 1,
        next => next,
    }
}

fn dump_report_descriptor(
    output_dir: &Path,
    prefix: &str,
    device: &hidapi::HidDevice,
) -> Result<()> {
    let mut descriptor = vec![0_u8; hidapi::MAX_REPORT_DESCRIPTOR_SIZE];
    let size = device
        .get_report_descriptor(&mut descriptor)
        .context("failed to read report descriptor")?;
    descriptor.truncate(size);

    let path = write_named_dump_file(output_dir, &format!("{prefix}_descriptor"), &descriptor)?;
    let report_ids = extract_report_ids(&descriptor);
    println!("  descriptor: {} bytes -> {}", size, path.display());
    if report_ids.is_empty() {
        println!("  descriptor report_ids: none");
    } else {
        println!(
            "  descriptor report_ids: {}",
            report_ids
                .iter()
                .map(|id| format!("{id:02X}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    Ok(())
}

fn probe_feature_reports(
    output_dir: &Path,
    prefix: &str,
    device: &hidapi::HidDevice,
) -> Result<()> {
    let mut descriptor = vec![0_u8; hidapi::MAX_REPORT_DESCRIPTOR_SIZE];
    let size = device
        .get_report_descriptor(&mut descriptor)
        .context("failed to read report descriptor")?;
    descriptor.truncate(size);

    let mut report_ids = extract_report_ids(&descriptor);
    if report_ids.is_empty() {
        report_ids.push(0);
    }

    for report_id in report_ids {
        let mut feature = vec![0_u8; 256];
        feature[0] = report_id;
        match device.get_feature_report(&mut feature) {
            Ok(size) if size > 0 => {
                feature.truncate(size);
                let path = write_named_dump_file(
                    output_dir,
                    &format!("{prefix}_feature_{report_id:02x}"),
                    &feature,
                )?;
                println!(
                    "  feature report {:02X}: {} bytes -> {}",
                    report_id,
                    size,
                    path.display()
                );
                println!("    hex={}", to_hex(&feature));
            }
            Ok(_) => {
                println!("  feature report {report_id:02X}: empty");
            }
            Err(error) => {
                println!("  feature report {report_id:02X}: failed: {error}");
            }
        }
    }

    Ok(())
}

fn write_named_dump_file(output_dir: &Path, prefix: &str, data: &[u8]) -> Result<PathBuf> {
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before unix epoch")?
        .as_millis();
    let path = output_dir.join(format!("{prefix}_{timestamp_ms}.bin"));
    fs::write(&path, data)?;
    info!("wrote hid dump {}", path.display());
    Ok(path)
}

fn extract_report_ids(descriptor: &[u8]) -> Vec<u8> {
    let mut ids = Vec::new();
    let mut index = 0;

    while index < descriptor.len() {
        let prefix = descriptor[index];
        index += 1;

        if prefix == 0xFE {
            if index + 1 >= descriptor.len() {
                break;
            }
            let size = descriptor[index] as usize;
            index += 2 + size;
            continue;
        }

        let size = match prefix & 0x03 {
            0 => 0,
            1 => 1,
            2 => 2,
            _ => 4,
        };

        if (prefix & 0xFC) == 0x84 && index < descriptor.len() {
            let report_id = descriptor[index];
            if !ids.contains(&report_id) {
                ids.push(report_id);
            }
        }

        index += size;
    }

    ids
}

fn feature_report_ids_for_descriptor(
    settings: &Settings,
    all_collections: bool,
    descriptor: &[u8],
) -> Vec<u8> {
    if !all_collections && !settings.feature_report_ids.is_empty() {
        return settings.feature_report_ids.clone();
    }

    let mut ids = extract_report_ids(descriptor);
    for id in &settings.feature_report_ids {
        if !ids.contains(id) {
            ids.push(*id);
        }
    }
    ids
}

fn create_state_dir(label: &str) -> Result<PathBuf> {
    let sanitized = sanitize_label(label);
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before unix epoch")?
        .as_millis();
    Ok(PathBuf::from("dumps")
        .join("states")
        .join(format!("{timestamp_ms}_{sanitized}")))
}

fn create_series_dir(label: &str) -> Result<PathBuf> {
    let sanitized = sanitize_label(label);
    let timestamp_ms = current_timestamp_ms()?;
    Ok(PathBuf::from("dumps")
        .join("series")
        .join(format!("{timestamp_ms}_{sanitized}")))
}

fn current_timestamp_ms() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before unix epoch")?
        .as_millis())
}

fn sanitize_label(label: &str) -> String {
    let sanitized = label
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "state".to_string()
    } else {
        sanitized
    }
}

fn list_state_files(root: &Path) -> Result<std::collections::BTreeMap<PathBuf, PathBuf>> {
    let mut files = std::collections::BTreeMap::new();
    collect_state_files(root, root, &mut files)?;
    Ok(files)
}

fn collect_state_files(
    root: &Path,
    current: &Path,
    files: &mut std::collections::BTreeMap<PathBuf, PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_state_files(root, &path, files)?;
        } else {
            let relative = path
                .strip_prefix(root)
                .context("failed to make relative state path")?
                .to_path_buf();
            files.insert(relative, path);
        }
    }

    Ok(())
}

#[derive(Debug)]
struct FeatureSeriesSample {
    sample_index: usize,
    data: Vec<u8>,
}

fn collect_feature_series_groups(
    root: &Path,
) -> Result<std::collections::BTreeMap<(String, u8), Vec<FeatureSeriesSample>>> {
    let mut files = std::collections::BTreeMap::new();
    collect_state_files(root, root, &mut files)?;

    let mut groups: std::collections::BTreeMap<(String, u8), Vec<FeatureSeriesSample>> =
        std::collections::BTreeMap::new();

    for (relative, absolute) in files {
        let Some(file_name) = relative.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some((report_id, sample_index)) = parse_feature_series_file_name(file_name) else {
            continue;
        };

        let device_key = relative
            .parent()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| ".".to_string());

        let data = fs::read(&absolute)
            .with_context(|| format!("failed to read {}", absolute.display()))?;
        groups
            .entry((device_key, report_id))
            .or_default()
            .push(FeatureSeriesSample { sample_index, data });
    }

    Ok(groups)
}

fn parse_feature_series_file_name(file_name: &str) -> Option<(u8, usize)> {
    let stem = file_name.strip_suffix(".bin")?;
    let rest = stem.strip_prefix("feature_")?;
    let (report_id, sample_index) = rest.split_once('_')?;
    let report_id = u8::from_str_radix(report_id, 16).ok()?;
    let sample_index = sample_index.parse().ok()?;
    Some((report_id, sample_index))
}

fn format_offset_ranges(offsets: &[usize]) -> String {
    if offsets.is_empty() {
        return "-".to_string();
    }

    let mut ranges = Vec::new();
    let mut start = offsets[0];
    let mut previous = offsets[0];

    for &offset in &offsets[1..] {
        if offset == previous + 1 {
            previous = offset;
        } else {
            ranges.push(format_range(start, previous));
            start = offset;
            previous = offset;
        }
    }
    ranges.push(format_range(start, previous));
    ranges.join(", ")
}

fn format_range(start: usize, end: usize) -> String {
    if start == end {
        format!("0x{start:04X}")
    } else {
        format!("0x{start:04X}-0x{end:04X}")
    }
}
