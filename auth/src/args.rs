#[derive(Debug)]
pub struct Args {
    pub new: bool,
}

pub fn parse() -> Result<Args, lexopt::Error> {
    use lexopt::prelude::*;

    let mut new = false;
    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Long("new") => {
                new = true;
            }
            Long("help") => {
                println!("Usage: rad auth [--new]");
                std::process::exit(0);
            }
            _ => return Err(arg.unexpected()),
        }
    }

    Ok(Args { new })
}
