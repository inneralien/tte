use anyhow::Result;
use csv::Trim;
use log::LevelFilter;
use log::{debug, error, info, warn};
use rust_decimal::prelude::*;
use serde::Deserialize;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::fs::File;
use std::io;
use std::process;

type Records = HashMap<u32, Decimal>;

/// Client data
///
/// This is the main structure for holding client acount balances.
/// * Assumption #1 - If an account is locked no future deposits/withdrawals are
/// allowed. There is no way to unlock an account once it is locked.
#[derive(Default)]
struct Client {
    /// Client records are a simple mapping from transaction id (`tx`) to
    /// transaction `amount.` They are used by dispute/resolve/chargeback
    /// transactions that reference `tx` to get an `amount.`
    records: Records,
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
    in_dispute: bool,
}

/// Custom [Debug] impl for [Client] so that the fields are shown without the
/// [Records] HashMap
/// ```
/// Client { available: 24.5  held: 2  total: 26.5  locked: false }
/// ```
impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Client {{ available: {}  held: {}  total: {}  locked: {} }}",
            self.available.round_dp(4),
            self.held.round_dp(4),
            self.total.round_dp(4),
            self.locked
        )
    }
}

impl fmt::Display for Client {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}, {}, {}, {}",
            self.available.round_dp(4),
            self.held.round_dp(4),
            self.total.round_dp(4),
            self.locked
        )
    }
}

impl Client {
    /// Add a mapping entry for a `tx` to an `amount`
    fn add_record(&mut self, tx: u32, amount: Decimal) -> Result<()> {
        debug!("  add record tx:{}  amount:{}", tx, amount);
        self.records.insert(tx, amount);
        Ok(())
    }

    /// Consumes a transaction provided by [read_csv] and performs the appropriate
    /// transaction task
    fn transact(&mut self, transaction: Transaction) -> Result<()> {
        match transaction.trans {
            TransType::Deposit => {
                if !self.locked {
                    if let Some(amount) = transaction.amount {
                        self.add_record(transaction.tx, amount)?;
                        self.deposit(amount)?;
                    } else {
                        error!("O_o No amount specified in Deposit transaction");
                    }
                }
            }
            TransType::Withdrawal => {
                if !self.locked {
                    if let Some(amount) = transaction.amount {
                        self.add_record(transaction.tx, amount)?;
                        self.withdrawal(amount)?;
                    } else {
                        error!("O_o No amount in withdrawn");
                    }
                }
            }
            TransType::Dispute => {
                self.dispute(transaction.tx)?;
            }
            TransType::Resolve => {
                if self.in_dispute {
                    self.resolve(transaction.tx)?;
                } else {
                    error!("client not in dispute");
                }
            }
            TransType::Chargeback => {
                if self.in_dispute {
                    self.chargeback(transaction.tx)?;
                } else {
                    error!("client not in dispute");
                }
            }
        };
        Ok(())
    }

    fn deposit(&mut self, amount: Decimal) -> io::Result<()> {
        debug!("  depositing: {}", amount);
        self.available += amount;
        self.total += amount;
        debug!("  {:?}", self);
        Ok(())
    }

    fn withdrawal(&mut self, amount: Decimal) -> io::Result<()> {
        if self.available >= amount {
            debug!("withdrawing: {}", amount);
            self.available -= amount;
            self.total -= amount;
            debug!("{}", self);
        } else {
            warn!("Insufficient funds for withdrawal");
        }
        Ok(())
    }

    fn dispute(&mut self, tx: u32) -> io::Result<()> {
        if let Some(amount) = self.records.get(&tx) {
            info!("Disputing tx:{tx} amount:{amount}");
            self.available -= amount;
            self.held += amount;
            self.in_dispute = true;
        } else {
            warn!("Could not find tx:{tx} to dispute. CSV data error?");
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
            warn!("Could not find tx:{tx} to resolve. CSV data error?");
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
            warn!("Could not find tx:{tx} to chargeback. CSV data error?");
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

// Currently only used by the unit tests
#[allow(dead_code)]
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
}

fn read_csv(csv: impl io::Read) -> csv::DeserializeRecordsIntoIter<impl io::Read, Transaction> {
    let rdr = csv::ReaderBuilder::new().trim(Trim::All).from_reader(csv);
    rdr.into_deserialize()
}

fn usage() {
    println!("Usage");
    println!("    cargo run -- transactions.cv > account.csv");
    process::exit(1);
}

fn main() -> Result<()> {
    env_logger::builder()
        .format_timestamp(None)
        .filter_level(LevelFilter::Info)
        .init();

    let mut clients: HashMap<u16, Client> = HashMap::new();

    if let Some(filename) = get_first_arg() {
        match File::open(filename) {
            Ok(open_file) => {
                let transactions = read_csv(open_file);
                for result in transactions {
                    let transaction: Transaction = result?;
                    debug!("{:?}", transaction);

                    if let Entry::Vacant(e) = clients.entry(transaction.client) {
                        debug!("  Adding new client: {}", transaction.client);
                        e.insert(Client::default());
                    } else {
                        debug!("  Client {} exists", transaction.client);
                    }

                    if let Some(client) = clients.get_mut(&transaction.client) {
                        client.transact(transaction)?;
                    }
                }
            }
            Err(e) => {
                error!("{}", e);
                usage();
            }
        };

        // Print out all the clients and their account info
        println!("client, available, held, total, locked");
        for (id, client) in clients {
            println!("{}, {}", id, client);
        }
    } else {
        usage();
    }

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

        client.deposit(dec!(1.0)).unwrap();
        client.deposit(dec!(2.0)).unwrap();
        client.withdrawal(dec!(1.5)).unwrap();
        assert_eq!(client.available, dec!(1.5));
        assert_eq!(client.held, dec!(0));
        assert_eq!(client.total, dec!(1.5));
        assert_eq!(client.locked, false);
    }

    #[test]
    fn test_withdrawal_insufficient_funds() {
        log_init();
        let mut client = Client::default();
        client.withdrawal(dec!(1.5)).unwrap();
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
        assert_eq!(client.total, amount + amount);
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
    fn test_transaction_chargeback() -> Result<()> {
        const DATA: &'static str = "\
type,client,tx,amount
deposit,1,1,1.0
deposit,1,2,2.0
deposit,1,3,100.0
dispute,1,3,
deposit,1,4,100.0
chargeback,1,3,
";
        let mut client = Client::default();
        let transactions = read_csv(DATA.as_bytes());
        for result in transactions {
            let transaction: Transaction = result?;
            client.transact(transaction)?;
        }
        assert_eq!(client.held, dec!(0));
        assert_eq!(client.total, dec!(103));
        assert_eq!(client.locked, true);
        assert_eq!(client.in_dispute, true);
        Ok(())
    }

    #[test]
    fn test_parse_csv_file() {
        let _ = OsString::from_str("transactions.csv").unwrap();
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

        // Dispute a withdrawal
        let record = Transaction::new(TransType::Dispute, 1, 2, None);
        println!("{:#?}", record);
        assert_eq!(client.held, dec!(0));
        assert!(client.transact(record).is_ok());
        assert_eq!(client.available, dec!(3));
        assert_eq!(client.total, dec!(6.5));
        assert_eq!(client.held, dec!(3.5));
        assert!(client.in_dispute);

        // Resolve the dispute
        let record = Transaction::new(TransType::Resolve, 1, 2, None);
        println!("{:?}", client);
        assert!(client.transact(record).is_ok());
        assert!(!client.in_dispute);
        assert_eq!(client.available, dec!(6.5));
        assert_eq!(client.total, dec!(6.5));
        assert_eq!(client.held, dec!(0));

        // Dispute another
        let record = Transaction::new(TransType::Dispute, 1, 1, None);
        assert!(client.transact(record).is_ok());

        // Chargeback
        let record = Transaction::new(TransType::Chargeback, 1, 1, None);
        assert!(client.transact(record).is_ok());
        println!("{:?}", client);
        assert!(client.in_dispute);
        assert!(client.locked);
        assert_eq!(client.held, dec!(0));
        // Since the dispute was on a withdrawal the total will be negative
        assert_eq!(client.total, dec!(-3.5));

        Ok(())
    }
}
