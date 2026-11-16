use std::process::ExitCode;

fn main() -> ExitCode {
    let schema = juliana_cal_core::schema();

    match serde_json::to_string_pretty(&schema) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
