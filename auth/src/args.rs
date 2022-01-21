#[derive(Debug)]
pub struct Args {
    pub init: bool,
}

pub fn parse() -> Result<Args, lexopt::Error> {
    use lexopt::prelude::*;

    let mut init = false;
    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Long("init") => {
                init = true;
            }
            Long("help") => {
                println!("Usage: rad auth [--init]");
                std::process::exit(0);
            }
            _ => return Err(arg.unexpected()),
        }
    }

    Ok(Args { init })
}
