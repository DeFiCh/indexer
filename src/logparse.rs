use crate::lang;
use crate::models;
use crate::models::LogEntry;
use crate::models::LogEntryMap;
use crate::models::LogIcxCalcData;
use crate::models::LogSwapData;
use clap::Parser;
use lang::Result;
use models::LogIcxData;
use std::io::BufRead;
use tracing::info;
use tracing::warn;

#[derive(Parser, Debug)]
pub struct LogParseArgs {
    // The path to the debug.log file from defid.
    // This can be both gzipped or raw file. If the file is gzipped
    // it will automatically be decompressed on the fly.
    #[arg(long, default_value = "data/debug.log.gz")]
    pub defid_log_path: String,
    #[arg(long, default_value = "ICX:")]
    pub log_icx_matcher: String,
    #[arg(long, default_value = "ICXCalc:")]
    pub log_icx_calc_matcher: String,
    #[arg(long, default_value = "SwapResult:")]
    pub log_swap_matcher: String,
}

pub fn run(args: &LogParseArgs) -> Result<()> {
    info!("{:?}", args);

    let defid_log_path = match args.defid_log_path.is_empty() {
        true => return Err(lang::Error::from("defid log path is empty")),
        false => args.defid_log_path.as_str(),
    };

    let quit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, std::sync::Arc::clone(&quit))?;

    let mut log_entry_map = LogEntryMap::new();

    info!("ingesting log file: {}", defid_log_path);

    process_log_file(
        defid_log_path,
        args.log_icx_matcher.as_str(),
        args.log_icx_calc_matcher.as_str(),
        args.log_swap_matcher.as_str(),
        &mut log_entry_map,
    )?;

    info!(
        "log file ingested:\n\
        \tTotal transactions:     {}\n\
        \tTotal ICX entries:      {}\n\
        \tTotal ICX calc entries: {}\n\
        \tTotal Swap entries:     {}",
        log_entry_map.data.len(),
        log_entry_map.icx_count,
        log_entry_map.icx_calc_count,
        log_entry_map.swap_count,
    );

    Ok(())
}

pub fn process_log_file(
    defid_log_path: &str,
    log_icx_matcher: &str,
    log_icx_calc_matcher: &str,
    log_swap_matcher: &str,
    combined_data: &mut LogEntryMap,
) -> Result<()> {
    let file = std::fs::File::open(defid_log_path)?;
    let mut reader: Box<dyn BufRead> = if defid_log_path.ends_with(".gz") {
        Box::new(std::io::BufReader::new(flate2::read::GzDecoder::new(file)))
    } else {
        Box::new(std::io::BufReader::new(file))
    };

    let mut line_buffer = String::new();

    fn parse_json_line<T>(line: &str) -> Option<T>
    where
        T: serde::de::DeserializeOwned,
    {
        line.find('{')
            .map(|start| &line[start..])
            .and_then(|json_str| match serde_json::from_str(json_str) {
                Ok(data) => Some(data),
                Err(_) => {
                    warn!("json parse failure: {}", json_str);
                    None
                }
            })
    }

    while reader.read_line(&mut line_buffer)? != 0 {
        match () {
            _ if line_buffer.contains(log_icx_matcher) => {
                if let Some(data) = parse_json_line::<LogIcxData>(&line_buffer) {
                    let entry = combined_data
                        .data
                        .entry(data.claim_tx.clone())
                        .or_insert_with(LogEntry::new);
                    entry.icx_data = Some(data);
                    combined_data.icx_count += 1;
                }
            }
            _ if line_buffer.contains(log_icx_calc_matcher) => {
                if let Some(data) = parse_json_line::<LogIcxCalcData>(&line_buffer) {
                    let entry = combined_data
                        .data
                        .entry(data.calc_tx.clone())
                        .or_insert_with(LogEntry::new);
                    entry.icx_calc_data = Some(data);
                    combined_data.icx_calc_count += 1;
                }
            }
            _ if line_buffer.contains(log_swap_matcher) => {
                if let Some(data) = parse_json_line::<LogSwapData>(&line_buffer) {
                    let entry = combined_data
                        .data
                        .entry(data.txid.clone())
                        .or_insert_with(LogEntry::new);
                    entry.swap_data = Some(data);
                    combined_data.swap_count += 1;
                }
            }
            _ => {}
        }
        line_buffer.clear();
    }

    Ok(())
}
