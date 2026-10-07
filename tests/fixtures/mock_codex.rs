// Test fixture only. Does not connect to any service or execute project commands.
use std::{env,fs};
fn q(value:&str)->String{format!("\"{}\"",value.replace('\\',"\\\\").replace('"',"\\\"").replace('\n',"\\n").replace('\r',"\\r").replace('\t',"\\t"))}
fn main(){
    let args:Vec<_>=env::args().skip(1).collect();
    if args.iter().any(|a|a=="--help"){println!("Mock Codex: --no-daemon --yolo --sandbox --ask-for-approval --search --no-alt-screen --worktree");return;}
    let Some(path)=env::var_os("SLOTSHIFT_PROBE_OUT") else{eprintln!("This executable is a local test fixture, not Codex.");std::process::exit(2);};
    let home=env::var("CODEX_HOME").unwrap_or_default();
    let cwd=env::current_dir().unwrap().to_string_lossy().into_owned();
    let clean=["OPENAI_API_KEY","CODEX_API_KEY","CODEX_ACCESS_TOKEN","OPENAI_BASE_URL","CODEX_THREAD_ID"].iter().all(|key|env::var_os(key).is_none());
    let body=format!("{{\"home\":{},\"cwd\":{},\"args\":[{}],\"api_environment_cleared\":{}}}",q(&home),q(&cwd),args.iter().map(|s|q(s)).collect::<Vec<_>>().join(","),clean);
    fs::write(path,body).unwrap();println!("SLOTSHIFT TEST PASSED: mock CLI reached. No model request was sent.");
}