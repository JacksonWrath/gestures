use miette::Result;
use regex::Regex;
use std::process::Command;

#[derive(Clone)]
pub struct ExecArgs {
    pub args: String,
    pub dx: f64,
    pub dy: f64,
    pub da: f64,
    pub scale: f64,
}

impl ExecArgs {
    pub fn new_args_only(args: String) -> ExecArgs {
        Self {
            args,
            dx: 0.0,
            dy: 0.0,
            da: 0.0,
            scale: 0.0,
        }
    }
}

pub fn exec_command(exec_args: &ExecArgs) -> Result<()> {
    exec_command_from_string(
        exec_args.args.as_str(),
        exec_args.dx,
        exec_args.dy,
        exec_args.da,
        exec_args.scale
    )
}

pub fn exec_command_from_string(args: &str, dx: f64, dy: f64, da: f64, scale: f64) -> Result<()> {
    if !&args.is_empty() {
        let args = args.to_string();
        std::thread::spawn(move || {
            let rx = Regex::new(r"[^\\]\$delta_x").unwrap();
            let ry = Regex::new(r"[^\\]\$delta_y").unwrap();
            let rs = Regex::new(r"[^\\]\$scale").unwrap();
            let ra = Regex::new(r"[^\\]\$delta_angle").unwrap();
            let args = ry.replace_all(&args, format!(" {dy} "));
            let args = rx.replace_all(&args, format!(" {dx} "));
            let args = rs.replace_all(&args, format!(" {scale} "));
            let args = ra.replace_all(&args, format!(" {da} "));
            log::debug!("{:?}", &args);
            Command::new("sh")
                .arg("-c")
                .arg(&*args)
                .spawn()
                .unwrap()
                .wait()
                .unwrap();
        });
    }
    Ok(())
}
