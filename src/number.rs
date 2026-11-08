use std::{cmp, ops};
use std::cmp::Ordering;
use std::fmt::Formatter;
use std::ops::BitAnd;
use std::str::FromStr;
use dashu::base::{ExtendedGcd, Sign, Signed};
use dashu::integer::UBig;
use thiserror::Error;
use crate::number::ArithmeticError::{NotImplemented, ZeroDivision};
use crate::number::Repr::*;

fn ione() -> dashu::Integer {
    dashu::Integer::ONE.clone()
}

#[derive(Error, Debug)]
pub enum ArithmeticError {
    #[error("BUG: bad integer representation: {0}")]
    BadIntegerString(String),
    #[error("cannot factorialize negative numbers")]
    NegativeFactorial,
    #[error("too big number to be factorialized (it's over 9 quintillions!)")]
    TooBigFactorial,
    #[error("raising to the power of {0} is a strong move. However, even this calculator has its boundaries. Congratulations, you've stumbled upon one of them.")]
    TooBigExponent(UBig),
    #[error("not yet implemented: {0}")]
    NotImplemented(&'static str),
    #[error("division by zero!")]
    ZeroDivision,
}

#[derive(Debug)]
pub enum Approx {
    Exact,
    ExactF64(f64),
    PeriodicF64(f64),
}

#[derive(Debug, Clone)]
pub enum Repr {
    Int(dashu::Integer),
    Decimal(dashu::Integer, dashu::Natural),
}

impl std::fmt::Display for Repr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
           Int(n) => n.to_string(),
           Decimal(p, q) => format!("{p}/{q}")
        }.as_str())
    }
}

impl TryFrom<&str> for Repr {
    type Error = ArithmeticError;

    fn try_from(num: &str) -> Result<Self, Self::Error> {
        if let Some(idx) = num.find(['e', 'E']) {
            let (l, r) = num.split_at(idx);
            Self::parse_scientific(l, &r[1..])
        } else if let Some((a, b)) = num.split_once('.') {
            Self::parse_decimal(a, b)
        } else if let Ok(n) = dashu::Integer::from_str(num) {
            Ok(Int(n))
        } else {
            Err(ArithmeticError::BadIntegerString(num.to_string()))
        }
    }
}

impl From<isize> for Repr {
    fn from(value: isize) -> Self {
        Int(dashu::Integer::from_i64(value as i64))
    }
}

impl From<(isize, usize)> for Repr {
    fn from(value: (isize, usize)) -> Self {
        Decimal(
            dashu::Integer::from_i64(value.0 as i64),
            dashu::Natural::from_u64(value.1 as u64),
        )
    }
}

impl From<bool> for Repr {
    fn from(b: bool) -> Self {
        Int(if b { dashu::Integer::ONE } else { dashu::Integer::ZERO })
    }
}

impl Repr {
    fn parse_decimal(num: &str, denom: &str) -> Result<Self, ArithmeticError> {
        if denom.is_empty() {
            return Self::try_from(num)
        }

        let p = match dashu::Integer::from_str(format!("{num}{denom}").as_str()) {
            Ok(n) => n,
            Err(_) => return Err(ArithmeticError::BadIntegerString(format!("{num}.{denom}")))
        };
        let q = (dashu::Natural::ONE*10u64).pow(denom.len());

        Ok(Decimal(p, q))
    }

    fn parse_scientific(num: &str, exp: &str) -> Result<Self, ArithmeticError> {
        Ok(Self::try_from(num)? * Repr::from(10).pow(Self::try_from(exp)?)?)
    }

    pub fn factorial(&self) -> Result<Self, ArithmeticError> {
        match self {
            Int(n) => {
                if n.is_negative() {
                    Err(ArithmeticError::NegativeFactorial)
                } else if let Ok(num) = n.try_into() {
                    Ok(Int(Self::isize_factorial(num)))
                } else {
                    Err(ArithmeticError::TooBigFactorial)
                }
            },
            Decimal(p, q) => {
                if q.is_one() {
                    Ok(Int(Self::integer_factorial(p)?))
                } else {
                    Err(NotImplemented("fractional factorials"))
                }
            },
        }
    }

    fn integer_factorial(n: &dashu::Integer) -> Result<dashu::Integer, ArithmeticError> {
        if n.is_negative() {
            Err(ArithmeticError::NegativeFactorial)
        } else if let Ok(num) = n.try_into() {
            Ok(Self::isize_factorial(num))
        } else {
            Err(ArithmeticError::TooBigFactorial)
        }
    }

    fn isize_factorial(n: isize) -> dashu::Integer {
        (2..=n).fold(dashu::Integer::ONE, |a, b| a*b)
    }

    pub fn normalize(self) -> Self {
        match self {
            Int(_) => self,
            Decimal(p, q) => {
                if q.is_one() {
                    Int(p)
                } else {
                    let (gcd, _, _) = (&p).gcd_ext(&q);
                    if gcd == q {
                        Int(p/gcd)
                    } else {
                        Decimal(p/&gcd, q/gcd)
                    }
                }
            }
        }
    }

    pub fn recip(&self) -> Result<Self, ArithmeticError> {
        match self {
            Int(n) => {
                if n.is_zero() {
                    Err(ZeroDivision)
                } else {
                    let (sign, ubig) = n.clone().into_parts();
                    Ok(Decimal(ione()*sign, ubig))
                }
            },
            Decimal(p, q) => {
                if p.is_zero() {
                    Err(ZeroDivision)
                } else {
                    let (sign, ubig) = p.clone().into_parts();
                    Ok(Decimal(q.clone()*sign, ubig))
                }
            }
        }
    }

    pub fn pow(&self, power: Self) -> Result<Self, ArithmeticError> {
        match power {
            Int(n) => {
                let (mut sign, ubig) = n.into_parts();
                if sign == Sign::Negative && (&ubig).bitand(1u64) == 0u64 {
                    sign = Sign::Positive;
                }
                
                if ubig.is_zero() {
                    return Ok(Int(ione()))
                }

                match (&ubig).try_into() {
                    Ok(n_usize) => {
                        let res = self.nat_pow(n_usize);
                        if sign == Sign::Negative { res.recip() } else { Ok(res) }
                    },
                    Err(_) => Err(ArithmeticError::TooBigExponent(ubig))
                }
            },
            Decimal(p, q) => {
                if !q.is_one() {
                    Err(NotImplemented("raising to a rational power"))
                } else {
                    self.pow(Int(p))
                }
            }
        }
    }

    fn nat_pow(&self, n: usize) -> Self {
        match self {
            Int(p) => Int(p.pow(n)),
            Decimal(p, q) => Decimal(p.pow(n), q.pow(n))
        }
    }

    pub fn approximate(&self) -> Approx {
        match self {
            Int(_) => Approx::Exact,
            Decimal(p, q) => {
                let (ratio, exact) = dashu::Rational::from_parts(
                    p.clone(),
                    q.clone(),
                ).to_f64().value_with_exact();
                
                if exact { Approx::ExactF64(ratio) } else { Approx::PeriodicF64(ratio) }
            }
        }
    }
}

impl ops::Add for &Repr {
    type Output = Repr;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Int(a), Int(b)) => Int(a+b),
            (Int(a), Decimal(p, q)) | (Decimal(p, q), Int(a)) => Decimal(p + a*q, q.clone()),
            (Decimal(p, q), Decimal(a, b)) => Decimal(p*b + a*q, b*q),
        }
    }
}

impl ops::Add for Repr {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        &self + &rhs
    }
}

impl ops::Neg for &Repr {
    type Output = Repr;

    fn neg(self) -> Self::Output {
        match self {
            Int(x) => Int(-x),
            Decimal(p, q) => Decimal(-p, q.clone())
        }
    }
}

impl ops::Neg for Repr {
    type Output = Self;

    fn neg(self) -> Self::Output {
        -&self
    }
}

impl ops::Sub for &Repr {
    type Output = Repr;

    fn sub(self, rhs: Self) -> Self::Output {
        self + &-rhs
    }
}

impl ops::Sub for Repr {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self + -rhs
    }
}

impl ops::Mul<Sign> for Repr {
    type Output = Repr;

    fn mul(self, rhs: Sign) -> Self::Output {
        match self {
            Int(n) => Int(n*rhs),
            Decimal(p, q) => Decimal(p*rhs, q)
        }
    }
}

impl ops::Mul for &Repr {
    type Output = Repr;

    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Int(a), Int(b)) => Int(a*b),
            (Int(a), Decimal(p, q)) | (Decimal(p, q), Int(a)) => Decimal(p*a, q.clone()),
            (Decimal(p, q), Decimal(a, b)) => Decimal(p*a, q*b)
        }
    }
}

impl ops::Mul for Repr {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        &self * &rhs
    }
}

impl ops::Div for &Repr {
    type Output = Repr;

    fn div(self, rhs: Self) -> Self::Output {
        self * &rhs.recip().unwrap()
    }
}

impl ops::Div for Repr {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        &self / &rhs
    }
}

impl cmp::PartialEq for Repr {
    fn eq(&self, other: &Self) -> bool {
        match self - other {
            Int(n)|Decimal(n, _) => n.is_zero(),
        }
    }
}

impl cmp::PartialEq<&Repr> for Repr {
    fn eq(&self, other: &&Self) -> bool {
        self == *other
    }
}

impl cmp::PartialOrd for &Repr {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match *self - *other {
            Int(n)|Decimal(n, _) => n.partial_cmp(&dashu::Integer::ZERO),
        }
    }
}

impl cmp::PartialOrd<&Repr> for Repr {
    fn partial_cmp(&self, other: &&Self) -> Option<Ordering> {
        (&self).partial_cmp(other)
    }
}

#[cfg(test)]
mod tests {
    use crate::number::{Approx, Repr};

    #[test]
    fn add_conversions() {
        assert_eq!(Repr::from(3),       Repr::from(1) + Repr::from(2));
        assert_eq!(Repr::from(-5),      Repr::from(0) - Repr::from(5));
        assert_eq!(Repr::from((1, 2)),  Repr::from(1) - Repr::from((1, 2)));
        assert_eq!(Repr::from((-1, 2)), Repr::from(1) - Repr::from((3, 2)));
    }

    #[test]
    fn normal() {
        assert_eq!(Repr::from(5), Repr::from(5).normalize());
        assert_eq!(Repr::from((1, 2)), Repr::from((2, 4)).normalize());
        assert_eq!(Repr::from((-1, 2)), Repr::from((-2, 4)).normalize());
    }
    
    #[test]
    fn approximate() {
        match Repr::from((1, 4)).approximate() {
            Approx::ExactF64(0.25) => {},
            otherwise => panic!("bad approximation: got {otherwise:?}, wanted Exact(0.25)")
        };
        match Repr::from((1, 3)).approximate() {
            Approx::PeriodicF64(0.32..0.34) => {},
            otherwise => panic!("bad approximation: got {otherwise:?}, wanted PeriodicF64(0.333...34)")
        };
    }

    #[test]
    fn equality() {
        assert_eq!(Repr::from(5), Repr::from(5));
        assert_eq!(Repr::from(1), Repr::from((4, 4)));
    }

    #[test]
    fn comparisons() {

    }

    #[test]
    fn multiplication() {
        assert_eq!(Repr::from(6),       Repr::from(2)       * Repr::from(3));
        assert_eq!(Repr::from((6, 15)), Repr::from(2)       * Repr::from((3, 15)));
        assert_eq!(Repr::from((6, 15)), Repr::from((3, 15)) * Repr::from(2));
        assert_eq!(Repr::from((6, 15)), Repr::from((2, 3))  * Repr::from((3, 5)));
    }

    #[test]
    fn formatter() {
        assert_eq!("5", Repr::from(5).to_string());
        assert_eq!("-5", Repr::from(-5).to_string());
        assert_eq!("1/2", Repr::from((1, 2)).to_string());
        assert_eq!("-1/2", Repr::from((-1, 2)).to_string());
    }
}
