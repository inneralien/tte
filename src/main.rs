use rust_decimal::prelude::*;

#[derive(Default, Debug)]
struct Client {
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
}

impl Client {
    fn deposit(&mut self, amount: Decimal) {
        self.available += amount;
        self.total += amount;
    }

}

fn main() {
    let a = Decimal::from_str("0.10055").unwrap();
    let b = Decimal::from_str("9.9").unwrap();
    let result_5 = (a + b).round_dp(5);
    let result_4 = (a + b).round_dp(4);
    println!("dp 5: {result_5}");
    println!("dp 4: {result_4}");

    let float_result_fwd: f32 = 0.10055 + 9.9;
    println!("Truncated to 5 places: {float_result_fwd:.5}");
    println!("Truncated to 4 places: {float_result_fwd:.4}");
//    let float_result_rev: f32 = 991.1 + 0.1001 + 0.5112 + 1.543 + 3.712 + 25.54 + 75.61 + 85.67 + 225.0 + 327.6;
//    println!("{float_result_rev:.4}");
}

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
    fn test_simple_deposit() {
        let mut client = Client::default();
        println!("{:?}", client);

        client.deposit(dec!(5.12));
        assert_eq!(client.available, dec!(5.12));
        assert_eq!(client.held, dec!(0));
        assert_eq!(client.total, dec!(5.12));
        assert_eq!(client.locked, false);
    }
}
