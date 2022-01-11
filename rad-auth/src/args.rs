#[derive(Debug)]
pub struct Args {
    pub add: bool,
}

pub fn parse() -> Result<Args, lexopt::Error> {
    use lexopt::prelude::*;

    let mut add = false;
    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Long("add") => {
                add = true;
            }
            Long("help") => {
                println!("Usage: hello [-n|--number=NUM] [--shout] THING");
                std::process::exit(0);
            }
            _ => return Err(arg.unexpected()),
        }
    }

    Ok(Args { add: add })
}
