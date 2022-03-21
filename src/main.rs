use rust_decimal::prelude::*;
use std::io;

enum ClientErrors {
    AccountLocked,
    InsufficientFunds,
}

// Client data
// Assumption #1 - If an account is locked no future deposits/withdrawls are
// allowed
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

fn main() {}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

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
}
