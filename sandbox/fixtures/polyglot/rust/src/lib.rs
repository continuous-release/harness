mod contract;
mod version;

pub fn message() -> &'static str {
    contract::MESSAGE
}

pub fn version() -> &'static str {
    version::VERSION
}

pub fn describe() -> String {
    format!("polyglot {} {}", version(), message())
}

#[cfg(test)]
mod tests {
    #[test]
    fn describes_the_contract() {
        assert!(super::describe().contains(super::message()));
    }
}
