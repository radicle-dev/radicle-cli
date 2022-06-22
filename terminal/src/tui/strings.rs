pub fn whitespaces(indent: u16) -> String {
    match String::from_utf8(vec![b' '; indent as usize]) {
        Ok(spaces) => spaces,
        Err(_) => String::new(),
    }
}
