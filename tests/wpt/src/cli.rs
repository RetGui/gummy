use crate::DEFAULT_WPT_DIR_NAME;
use crate::default_wpt_dir;
use anyhow::{anyhow, bail};
use std::env;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Args {
    pub mode: RunMode,
    pub ahem_font: Option<PathBuf>,
    pub wpt_dir: PathBuf,
    pub skip_download: bool,
}

#[derive(Debug)]
pub enum RunMode {
    AllCss { filter: Option<String> },
    Pair { test: PathBuf, reference: PathBuf },
}

impl Args {
    pub fn parse() -> anyhow::Result<Self> {
        let mut positional = Vec::new();
        let mut ahem_font = None;
        let mut wpt_dir = default_wpt_dir();
        let mut skip_download = false;
        let mut filter = None;
        let mut args = env::args().skip(1);

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => {
                    print_usage();
                    std::process::exit(0);
                }
                "--ahem-font" => {
                    ahem_font = Some(args.next().ok_or_else(|| anyhow!("--ahem-font requires a path"))?.into());
                }
                "--wpt-dir" => {
                    wpt_dir = args.next().ok_or_else(|| anyhow!("--wpt-dir requires a path"))?.into();
                }
                "--skip-download" => skip_download = true,
                "--filter" => {
                    filter = Some(args.next().ok_or_else(|| anyhow!("--filter requires a value"))?);
                }
                option if option.starts_with('-') => bail!("unknown option {option}"),
                _ => positional.push(arg.into()),
            }
        }

        let mode = match positional.len() {
            0 => RunMode::AllCss { filter },
            2 => RunMode::Pair { test: positional.remove(0), reference: positional.remove(0) },
            _ => bail!("Usage: wpt [OPTIONS] [TEST.html REF.html]"),
        };

        Ok(Self { mode, ahem_font, wpt_dir, skip_download })
    }
}

fn print_usage() {
    println!(
        "Usage: wpt [OPTIONS] [TEST.html REF.html]\n\
         \n\
         With no positional args, downloads WPT if needed and runs CSS reftests.\n\
         \n\
         Options:\n\
           --wpt-dir PATH       WPT checkout path (default: tests/wpt/{DEFAULT_WPT_DIR_NAME})\n\
           --skip-download      Do not auto-clone WPT when missing\n\
           --filter TEXT        Only run CSS reftests whose path contains TEXT\n\
           --ahem-font PATH     Override Ahem.ttf path"
    );
}
