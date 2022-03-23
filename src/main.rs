use anyhow::{Context, Result};
use csv::{ReaderBuilder, StringRecord, Trim};
use log::{debug, error, info, warn};
use rust_decimal::prelude::*;
use serde::Deserialize;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::process;

enum ClientErrors {
    AccountLocked,
    InsufficientFunds,
}

/// Client records are a simple mapping from transaction id (tx) to amount.
/// They are used by dispute transactions that reference `tx` to get an amount.
type Records = HashMap<u32, Decimal>;

// Client data
// Assumption #1 - If an account is locked no future deposits/withdrawals are
// allowed. There is no way to unlock an account once it is locked.
#[derive(Default, Debug)]
struct Client {
    records: Records,
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
    in_dispute: bool,
}

impl Client {
    fn add_record(&mut self, tx: u32, amount: Decimal) -> Result<()> {
        debug!("add record tx:{}  amount:{}", tx, amount);
        self.records.insert(tx, amount);
        Ok(())
    }

    fn transact(&mut self, transaction: Transaction) -> Result<()> {
        match transaction.trans {
            TransType::Deposit => {
                if !self.locked {
                    if let Some(amount) = transaction.amount {
                        self.add_record(transaction.tx, amount.into())?;
                        self.deposit(amount)?;
                    } else {
                        warn!("No amount specified in transaction");
                    }
                }
            }
            TransType::Withdrawal => {
                if !self.locked {
                    println!(">> withdrawal <<");
                    if let Some(amount) = transaction.amount {
                        self.withdrawal(amount)?;
                    } else {
                        println!("INFO: No amount in withdrawn");
                    }
                }
            }
            TransType::Dispute => {
                error!(">> dispute <<");
                self.dispute(transaction.tx)?;
                //                let thing = self.records.get(&tx);
                //                if let Some(amount) = thing { //self.records.get(&tx) {
                //                    println!("Disputing amount of {}", amount);
                //                        let amount = amount;
                //                    self.dispute(amount)?;
                //                } else {
                //                    println!("No records found")
                //                }
            }
            TransType::Resolve => println!(">> resolve <<"),
            TransType::Chargeback => println!(">> chargeback <<"),
        };
        Ok(())
    }

    fn deposit(&mut self, amount: Decimal) -> io::Result<()> {
        self.available += amount;
        self.total += amount;
        Ok(())
    }

    fn withdrawal(&mut self, amount: Decimal) -> io::Result<()> {
        self.available -= amount;
        self.total -= amount;
        Ok(())
    }

    fn dispute(&mut self, tx: u32) -> io::Result<()> {
        if let Some(amount) = self.records.get(&tx) {
            info!("dispute tx:{tx} amount:{amount}");
            self.available -= amount;
            self.held += amount;
            self.in_dispute = true;
        } else {
            error!("no amount found for tx:{tx}");
        };
        Ok(())
    }

    fn resolve(&mut self, tx: u32) -> io::Result<()> {
        if let Some(amount) = self.records.get(&tx) {
            info!("resolve tx:{tx} amount:{amount}");
            self.available += amount;
            self.held -= amount;
            self.in_dispute = false;
        } else {
            error!("no amount found for tx:{tx}");
        };
        Ok(())
    }

    fn chargeback(&mut self, tx: u32) -> io::Result<()> {
        if let Some(amount) = self.records.get(&tx) {
            info!("chargeback tx:{tx} amount:{amount}");
            self.locked = true;
            self.held -= amount;
            self.total -= amount;
        } else {
            error!("no amount found for tx:{tx}");
        };
        Ok(())
    }
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum TransType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Transaction {
    #[serde(rename = "type")]
    trans: TransType,
    client: u16,
    tx: u32,
    amount: Option<Decimal>,
}

impl Transaction {
    fn new(trans: TransType, client: u16, tx: u32, amount: Option<Decimal>) -> Transaction {
        Transaction {
            trans,
            client,
            tx,
            amount,
        }
    }
}
/// Taken from <https://docs.rs/csv/latest/csv/tutorial/index.html#reading-csv>
/// Returns the first positional argument sent to this process. If there are no
/// positional arguments, then this returns an error.
fn get_first_arg() -> Option<OsString> {
    env::args_os().nth(1)
    //    match env::args_os().nth(1) {
    //        None => Err(From::from("expected 1 argument, e.g. transactions.csv")),
    //        Some(file_path) => Ok(file_path),
    //    }
}

fn read_csv(csv: impl io::Read) -> csv::DeserializeRecordsIntoIter<impl io::Read, Transaction> {
    let rdr = csv::ReaderBuilder::new().trim(Trim::All).from_reader(csv);
    rdr.into_deserialize()
    //    for result in rdr.deserialize() {
    //        let transaction: Transaction = result?;
    //        println!("{:#?}", transaction);
    //    }
    //    Ok(())
}

//fn transact(
//    transactions: csv::DeserializeRecordsIntoIter<impl io::Read, Transaction>,
//) -> Result<Accounts> {
//    for transaction in transactions {
//        let record: Transaction = transaction?;
//        println!("{:?}", record);
//    }
//}

fn usage() {
    println!("Usage");
    println!("    cargo run -- transactions.cv > account.csv");
    process::exit(1);
}

fn main() -> Result<()> {
    env_logger::builder()
    .format_timestamp(None)
    .init();
    info!("an info");
    warn!("a warn");
    error!("an error");
    debug!("a debug");

    //    let filename = match get_first_arg() {
    //        Ok(filename) => filename,
    //        Err(error) => {
    //            println!("{}", error);
    //            process::exit(1);
    //        }
    //    };
    let mut clients: HashMap<u16, Client> = HashMap::new();

    if let Some(filename) = get_first_arg() {
        match File::open(filename) {
            Ok(open_file) => {
                let transactions = read_csv(open_file);
                for result in transactions {
                    let transaction: Transaction = result?;
                    debug!("{:#?}", transaction);

                    if let Entry::Vacant(e) = clients.entry(transaction.client) {
                        info!("Adding new client: {}", transaction.client);
                        e.insert(Client::default());
                    } else {
                        info!("Client {} exists", transaction.client);
                    }

                    if let Some(client) = clients.get_mut(&transaction.client) {
                        client.transact(transaction)?;
                    }

                    //                    if clients.contains_key(&transaction.client) {
                    //                        println!("Client {} exists", transaction.client);
                    //                    } else {
                    //                        println!("Adding new client: {}", transaction.client);
                    //                        clients.insert(transaction.client, Client::default());
                    //                    }
                }
            }
            Err(e) => {
                println!("{}", e);
                usage();
            }
        };

        for client in clients {
            println!("{:#?}", client);
        }
    } else {
        usage();
    }

    //    if let Some(result) = transactions.next() {
    //        let record: Transaction = result?;
    //        println!("{:?}", record);
    //    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
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

    fn log_init() {
        let _ = env_logger::builder()
        .format_timestamp(None)
        .is_test(true)
        .try_init();
    }

    #[test]
    fn test_client_defaults() {
        log_init();
        let client = Client::default();
        println!("{:?}", client);

        assert_eq!(client.available, dec!(0.0000));
        assert_eq!(client.held, dec!(0.0000));
        assert_eq!(client.total, dec!(0.0000));
        assert_eq!(client.locked, false);
    }

    #[test]
    fn test_basic_deposit() {
        log_init();
        let mut client = Client::default();
        println!("{:?}", client);

        client.deposit(dec!(3.14)).unwrap();
        assert_eq!(client.available, dec!(3.14));
        assert_eq!(client.held, dec!(0));
        assert_eq!(client.total, dec!(3.14));
        assert_eq!(client.locked, false);
    }

    #[test]
    fn test_basic_withdrawal() {
        log_init();
        let mut client = Client::default();
        println!("{:?}", client);

        client.deposit(dec!(3.14)).unwrap();
        client.withdrawal(dec!(3.14)).unwrap();
        assert_eq!(client.available, dec!(0));
        assert_eq!(client.held, dec!(0));
        assert_eq!(client.total, dec!(0));
        assert_eq!(client.locked, false);
    }

    #[test]
    fn test_basic_dispute() -> Result<()> {
        log_init();
        let mut client = Client::default();
        println!("{:#?}", client);

        let amount: Decimal = dec!(6.62);
        client.deposit(amount).unwrap();
        client.add_record(1, dec!(6.62))?;
        client.dispute(1).unwrap();
        assert_eq!(client.available, dec!(0));
        assert_eq!(client.held, amount);
        assert_eq!(client.total, amount);
        assert_eq!(client.locked, false);
        assert_eq!(client.in_dispute, true);
        Ok(())
    }

    #[test]
    fn test_basic_resolve() -> Result<()> {
        log_init();
        let mut client = Client::default();
        print!("{:#?}", client);

        let amount: Decimal = dec!(6.02);
        client.deposit(amount).unwrap();
        client.add_record(1, amount)?;
        client.dispute(1).unwrap();
        assert_eq!(client.available, dec!(0));
        assert_eq!(client.held, amount);
        assert_eq!(client.total, amount);
        assert_eq!(client.locked, false);
        assert_eq!(client.in_dispute, true);

        client.resolve(1).unwrap();
        assert_eq!(client.held, dec!(0));
        assert_eq!(client.available, amount);
        assert_eq!(client.total, amount);
        assert_eq!(client.locked, false);
        assert_eq!(client.in_dispute, false);

        Ok(())
    }

    #[test]
    fn test_basic_chargeback() -> Result<()> {
        log_init();
        let mut client = Client::default();
        print!("{:#?}", client);

        let amount: Decimal = dec!(6.28);
        client.deposit(amount).unwrap();
        client.deposit(amount).unwrap();
        client.add_record(1, amount)?;
        client.add_record(2, amount)?;
        client.dispute(2).unwrap();
        assert_eq!(client.available, amount);
        assert_eq!(client.held, amount);
        assert_eq!(client.total, amount+amount);
        assert_eq!(client.locked, false);
        assert_eq!(client.in_dispute, true);

        client.chargeback(2).unwrap();
        assert_eq!(client.available, amount);
        assert_eq!(client.held, dec!(0));
        assert_eq!(client.total, amount);
        assert_eq!(client.locked, true);
        assert_eq!(client.in_dispute, true);

        Ok(())
    }

    #[test]
    fn test_parse_csv_spaces() {
        read_csv(DATA_SPACES.as_bytes());
    }

    #[test]
    fn test_parse_csv_no_spaces() {
        read_csv(DATA_NO_SPACES.as_bytes());
    }

    #[test]
    fn test_parse_csv_no_header_fails() {
        //        assert!(read_csv(DATA_NO_HEADER.as_bytes()).is_err());
    }

    #[test]
    fn test_parse_csv_file() {
        let filename = OsString::from_str("transactions.csv").unwrap();
        //       assert!(read_csv(File::open(filename).unwrap()).is_ok());
    }

    #[test]
    fn test_csv_to_transactions() -> Result<()> {
        let mut transactions = read_csv(DATA_SPACES.as_bytes());

        if let Some(result) = transactions.next() {
            let record: Transaction = result?;
            assert_eq!(
                record,
                Transaction {
                    trans: TransType::Deposit,
                    client: 1,
                    tx: 1,
                    amount: Some(dec!(1.0)),
                }
            );
        }
        Ok(())
    }

    #[test]
    fn test_transact() -> Result<()> {
        //        const DATA: &'static str = "\
        //    type,       client,    tx,     amount
        //    deposit,         1,     1,       10.0
        //    withdrawal,      1,     2,        3.5
        //    dispute,         1,     2,
        //    resolve,         1,     2,
        //    ";
        //        let mut transactions = read_csv(DATA.as_bytes());
        let mut client = Client::default();

        // Deposit
        let record = Transaction::new(TransType::Deposit, 1, 1, Some(dec!(10.0)));
        println!("{:#?}", record);
        assert!(client.transact(record).is_ok());
        assert_eq!(client.available, dec!(10));

        // Withdrawl
        let record = Transaction::new(TransType::Withdrawal, 1, 2, Some(dec!(3.5)));
        println!("{:#?}", record);
        assert!(client.transact(record).is_ok());
        assert_eq!(client.available, dec!(6.5));

        // Dispute
        let record = Transaction::new(TransType::Dispute, 1, 2, None);
        println!("{:#?}", record);
        assert!(client.transact(record).is_ok());
        assert_eq!(client.available, dec!(2));
        assert_eq!(client.total, dec!(6.5));
        assert!(client.in_dispute);

        Ok(())
    }
}

// type,       client,     tx,     amount
// deposit,         1,     1,         1.0
// If client[1] does not exist then
//   client[1]::new()
// client[1].deposit(amount)

// deposit,         2,     2,         2.0
// if client[2] does not exist then
//   client[2]::new()
// client[2].deposit(amount)

// dispute,         1,     1,
// if
// 1 Client struct per client id
// Each client maintains a list of transactions with type,tx,amount
// client_transactions.get(3) -> Client0
// client_transactions.get(5) -> Client2
// deposit,         1,     3,         2.0
// withdrawal,      1,     4,         1.5
// withdrawal,      2,     5,         3.0
