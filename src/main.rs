use csv::{ReaderBuilder, StringRecord, Trim};
use rust_decimal::prelude::*;
use serde::Deserialize;
use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fs::File;
use std::io;

enum ClientErrors {
    AccountLocked,
    InsufficientFunds,
}

// Client data
// Assumption #1 - If an account is locked no future deposits/withdrawls are
// allowed. There is no way to unlock an account once it is locked.
#[derive(Default, Debug)]
struct Client {
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
}

impl Client {
    fn deposit(&mut self, amount: Decimal) -> io::Result<()> {
        self.available += amount;
        self.total += amount;
        Ok(())
    }

    fn withdrawl(&mut self, amount: Decimal) -> io::Result<()> {
        self.available -= amount;
        self.total -= amount;
        Ok(())
    }

    fn dispute(&mut self, amount: Decimal) -> io::Result<()> {
        self.available -= amount;
        self.held += amount;
        Ok(())
    }

    fn resolve(&mut self, amount: Decimal) -> io::Result<()> {
        self.held -= amount;
        self.available += amount;
        Ok(())
    }

    fn chargeback(&mut self, amount: Decimal) -> io::Result<()> {
        self.locked = true;
        self.held -= amount;
        self.total -= amount;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct Transaction {
    #[serde(rename = "type")]
    trans: String,
    client: u16,
    tx: u32,
    amount: Option<Decimal>,
}

/// Taken from <https://docs.rs/csv/latest/csv/tutorial/index.html#reading-csv>
/// Returns the first positional argument sent to this process. If there are no
/// positional arguments, then this returns an error.
fn get_first_arg() -> Result<OsString, Box<dyn Error>> {
    match env::args_os().nth(1) {
        None => Err(From::from("expected 1 argument, e.g. transactions.csv")),
        Some(file_path) => Ok(file_path),
    }
}

fn read_csv(csv: impl io::Read) -> Result<(), Box<dyn Error>> {
    let mut rdr = csv::ReaderBuilder::new().trim(Trim::All).from_reader(csv);
    for result in rdr.deserialize() {
        let transaction: Transaction = result?;
        println!("{:#?}", transaction);
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    //    let filename = match get_first_arg() {
    //        Ok(filename) => filename,
    //        Err(error) => {
    //            println!("{}", error);
    //            process::exit(1);
    //        }
    //    };

    let filename = get_first_arg()?;
    read_csv(File::open(filename)?)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    const DATA_SPACES: &'static str = "\
type,       client,     tx,     amount
deposit,         1,     1,         1.0
deposit,         2,     2,         2.0
deposit,         1,     3,         2.0
withdrawal,      1,     4,         1.5
withdrawal,      2,     5,         3.0
";

    const DATA_NO_SPACES: &'static str = "\
type,client,tx,amount
deposit,1,1,1.0
deposit,2,2,2.0
deposit,1,3,2.0
withdrawal,1,4,1.5
withdrawal,2,5,3.0
";

    const DATA_NO_HEADER: &'static str = "\
deposit,1,1,1.0
deposit,2,2,2.0
";

    #[test]
    fn test_client_defaults() {
        let client = Client::default();
        println!("{:?}", client);

        assert_eq!(client.available, dec!(0.0000));
        assert_eq!(client.held, dec!(0.0000));
        assert_eq!(client.total, dec!(0.0000));
        assert_eq!(client.locked, false);
    }

    #[test]
    fn test_basic_deposit() {
        let mut client = Client::default();
        println!("{:?}", client);

        client.deposit(dec!(3.14)).unwrap();
        assert_eq!(client.available, dec!(3.14));
        assert_eq!(client.held, dec!(0));
        assert_eq!(client.total, dec!(3.14));
        assert_eq!(client.locked, false);
    }

    #[test]
    fn test_basic_withdrawl() {
        let mut client = Client::default();
        println!("{:?}", client);

        client.deposit(dec!(3.14)).unwrap();
        client.withdrawl(dec!(3.14)).unwrap();
        assert_eq!(client.available, dec!(0));
        assert_eq!(client.held, dec!(0));
        assert_eq!(client.total, dec!(0));
        assert_eq!(client.locked, false);
    }

    #[test]
    fn test_basic_dispute() {
        let mut client = Client::default();
        print!("{:?}", client);

        let amount: Decimal = dec!(6.62);
        client.deposit(amount).unwrap();
        client.dispute(amount).unwrap();
        assert_eq!(client.available, dec!(0));
        assert_eq!(client.held, amount);
        assert_eq!(client.total, amount);
        assert_eq!(client.locked, false);
    }

    #[test]
    fn test_basic_resolve() {
        let mut client = Client::default();
        print!("{:?}", client);

        let amount: Decimal = dec!(6.02);
        client.deposit(amount).unwrap();
        client.dispute(amount).unwrap();
        client.resolve(amount).unwrap();
        assert_eq!(client.held, dec!(0));
        assert_eq!(client.available, amount);
        assert_eq!(client.total, amount);
        assert_eq!(client.locked, false);
    }

    #[test]
    fn test_basic_chargeback() {
        let mut client = Client::default();
        print!("{:?}", client);

        let amount: Decimal = dec!(6.28);
        client.deposit(amount + amount).unwrap();
        client.dispute(amount).unwrap();
        client.chargeback(amount).unwrap();
        assert_eq!(client.available, amount);
        assert_eq!(client.held, dec!(0));
        assert_eq!(client.total, amount);
        assert_eq!(client.locked, true);
    }

    #[test]
    fn test_parse_csv_spaces() {
        read_csv(DATA_SPACES.as_bytes()).unwrap();
    }

    #[test]
    fn test_parse_csv_no_spaces() {
        read_csv(DATA_NO_SPACES.as_bytes()).unwrap();
    }

    #[test]
    fn test_parse_csv_no_header_fails() {
        assert!(read_csv(DATA_NO_HEADER.as_bytes()).is_err());
    }

    #[test]
    fn test_parse_csv_file() {
        let filename = OsString::from_str("transactions.csv").unwrap();
        assert!(read_csv(File::open(filename).unwrap()).is_ok());
    }
}
