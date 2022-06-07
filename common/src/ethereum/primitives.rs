use anyhow::anyhow;

use ethers::types::U256;

pub fn amount_to_u256(amount: &str, decimals: usize) -> anyhow::Result<Option<U256>> {
    let decimal_position = amount.chars().position(|c| c == '.');
    if decimal_position.is_none() {
        return Ok(U256::from_dec_str(amount)
            .map_err(|_| anyhow!("Couldn't turn amount to U256"))?
            .checked_mul(U256::exp10(decimals)));
    }

    let decimal_position = decimal_position.unwrap();
    let mut fraction_length = amount.len() - decimal_position - 1;
    let mut amount = amount.to_string();
    amount.remove(decimal_position);

    if decimals < fraction_length {
        let extra_fraction_digits = fraction_length - decimals;
        amount = amount[..amount.len() - extra_fraction_digits].to_string();
        fraction_length -= extra_fraction_digits;
    }

    Ok(U256::from_dec_str(&amount)?.checked_mul(U256::exp10(decimals - fraction_length)))
}

pub fn u256_to_amount(amount: U256, decimals: usize) -> anyhow::Result<String> {
    let amount = amount.as_u128().to_string();
    let integer = if amount.len() <= decimals {
        "0"
    } else {
        &amount[..amount.len() - decimals]
    };
    let fraction = if amount.len() <= decimals {
        format!("{}{}", "0".repeat(decimals - amount.len()), amount)
    } else {
        amount[amount.len() - decimals..].to_string()
    };
    let last_fraction_pos = fraction.chars().rev().position(|c| c != '0');
    if let Some(pos) = last_fraction_pos {
        let fraction = &fraction[..fraction.len() - pos];
        Ok(format!("{}.{}", integer, fraction))
    } else {
        Ok(format!("{}.0", integer))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_string_to_u256() {
        assert_eq!(amount_to_u256("0.001", 3).unwrap().unwrap(), U256::from(1));
        assert_eq!(amount_to_u256("1", 3).unwrap().unwrap(), U256::from(1000));
        assert_eq!(amount_to_u256("1.", 3).unwrap().unwrap(), U256::from(1000));
        assert_eq!(
            amount_to_u256("1.234", 3).unwrap().unwrap(),
            U256::from(1234)
        );
        assert_eq!(
            amount_to_u256("1.2345", 3).unwrap().unwrap(),
            U256::from(1234)
        );
        assert_eq!(amount_to_u256(".12", 3).unwrap().unwrap(), U256::from(120));
        assert_eq!(amount_to_u256(".120", 3).unwrap().unwrap(), U256::from(120));
        assert_eq!(
            amount_to_u256(".1200", 3).unwrap().unwrap(),
            U256::from(120)
        );
        assert_eq!(
            amount_to_u256(".12000", 3).unwrap().unwrap(),
            U256::from(120)
        );
    }

    #[test]
    fn test_u256_to_amounting() {
        assert_eq!(u256_to_amount(U256::from(1), 3).unwrap(), "0.001");
        assert_eq!(u256_to_amount(U256::from(10001), 3).unwrap(), "10.001");
        assert_eq!(u256_to_amount(U256::from(1000), 3).unwrap(), "1.0");
        assert_eq!(u256_to_amount(U256::from(1100), 3).unwrap(), "1.1");
        assert_eq!(u256_to_amount(U256::from(1010), 3).unwrap(), "1.01");
        assert_eq!(u256_to_amount(U256::from(1011), 3).unwrap(), "1.011");
        assert_eq!(u256_to_amount(U256::from(1111), 3).unwrap(), "1.111");
        assert_eq!(u256_to_amount(U256::from(11111), 3).unwrap(), "11.111");
    }

    quickcheck! {
        fn quickcheck_u256_to_amount_to_u256(num: u128) -> bool {
            let decimals = 18_usize;
            num == amount_to_u256(&u256_to_amount(U256::from(num), decimals).unwrap(), decimals).unwrap().unwrap().as_u128()
        }
    }
}
