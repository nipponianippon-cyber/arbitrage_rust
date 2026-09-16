use crate::errors::AppError;
use num_bigint::BigUint;
use num_traits::{ToPrimitive, Zero};

pub(super) fn error(message: &str) -> AppError { AppError::Pricing(message.into()) }

// 中間積はu128を超えるため多倍長整数を使い、最後にだけ出力量へ変換する。
pub(super) fn div(n: BigUint, d: BigUint, up: bool) -> Result<BigUint, AppError> {
    if d.is_zero() { return Err(error("division by zero in local quote")); }
    Ok(if up && !n.is_zero() { (n + &d - 1u8) / d } else { n / d })
}

pub(super) fn mul_div(a: u128, b: u128, d: u128, up: bool) -> Result<u128, AppError> {
    div(BigUint::from(a) * b, BigUint::from(d), up)?.to_u128()
        .ok_or_else(|| error("local quote amount overflow"))
}

pub(super) fn to_u64(value: u128) -> Result<u64, AppError> {
    u64::try_from(value).map_err(|_| error("local quote exceeds SPL token amount"))
}

pub(super) fn bytes<const N: usize>(data: &[u8], offset: usize) -> Result<[u8; N], AppError> {
    data.get(offset..offset + N).and_then(|s| s.try_into().ok())
        .ok_or_else(|| error("truncated local quote account"))
}

pub(super) fn u16_at(d: &[u8], o: usize) -> Result<u16, AppError> { Ok(u16::from_le_bytes(bytes(d, o)?)) }
pub(super) fn u32_at(d: &[u8], o: usize) -> Result<u32, AppError> { Ok(u32::from_le_bytes(bytes(d, o)?)) }
pub(super) fn i32_at(d: &[u8], o: usize) -> Result<i32, AppError> { Ok(i32::from_le_bytes(bytes(d, o)?)) }
pub(super) fn u64_at(d: &[u8], o: usize) -> Result<u64, AppError> { Ok(u64::from_le_bytes(bytes(d, o)?)) }
pub(super) fn i64_at(d: &[u8], o: usize) -> Result<i64, AppError> { Ok(i64::from_le_bytes(bytes(d, o)?)) }
pub(super) fn u128_at(d: &[u8], o: usize) -> Result<u128, AppError> { Ok(u128::from_le_bytes(bytes(d, o)?)) }

pub(super) const Q64: u128 = 1u128 << 64;

// Orcaのtick定義に従い、正tickはQ96、負tickはQ64で各積を切り捨てる。
// 定数の出典はPLANS.mdのマイルストーン10根拠メモを参照。
pub(super) fn tick_sqrt(tick: i32) -> Result<u128, AppError> {
    if !(-443636..=443636).contains(&tick) { return Err(error("tick out of bounds")); }
    const POS: [u128; 19] = [
        79232123823359799118286999567, 79236085330515764027303304731,
        79244008939048815603706035061, 79259858533276714757314932305,
        79291567232598584799939703904, 79355022692464371645785046466,
        79482085999252804386437311141, 79736823300114093921829183326,
        80248749790819932309965073892, 81282483887344747381513967011,
        83390072131320151908154831281, 87770609709833776024991924138,
        97234110755111693312479820773, 119332217159966728226237229890,
        179736315981702064433883588727, 407748233172238350107850275304,
        2098478828474011932436660412517, 55581415166113811149459800483533,
        38992368544603139932233054999993551,
    ];
    const NEG: [u128; 19] = [
        18445821805675392311, 18444899583751176498, 18443055278223354162,
        18439367220385604838, 18431993317065449817, 18417254355718160513,
        18387811781193591352, 18329067761203520168, 18212142134806087854,
        17980523815641551639, 17526086738831147013, 16651378430235024244,
        15030750278693429944, 12247334978882834399, 8131365268884726200,
        3584323654723342297, 696457651847595233, 26294789957452057, 37481735321082,
    ];
    let shift = if tick >= 0 { 96usize } else { 64usize };
    let factors = if tick >= 0 { &POS } else { &NEG };
    let mut ratio = BigUint::from(1u8) << shift;
    for (bit, factor) in factors.iter().enumerate() {
        if tick.unsigned_abs() & (1u32 << bit) != 0 { ratio = (ratio * *factor) >> shift; }
    }
    if tick >= 0 { ratio >>= 32usize; }
    ratio.to_u128().ok_or_else(|| error("tick sqrt overflow"))
}

pub(super) fn delta_a(liquidity: u128, lo: u128, hi: u128, up: bool) -> Result<u128, AppError> {
    if hi < lo { return Err(error("invalid sqrt price interval")); }
    div(BigUint::from(liquidity) * (hi - lo) * Q64, BigUint::from(lo) * hi, up)?
        .to_u128().ok_or_else(|| error("token A delta overflow"))
}

pub(super) fn delta_b(liquidity: u128, lo: u128, hi: u128, up: bool) -> Result<u128, AppError> {
    if hi < lo { return Err(error("invalid sqrt price interval")); }
    mul_div(liquidity, hi - lo, Q64, up)
}

pub(super) fn next_sqrt(liquidity: u128, sqrt: u128, input: u128, a_to_b: bool) -> Result<u128, AppError> {
    if a_to_b {
        div(BigUint::from(liquidity) * sqrt * Q64,
            BigUint::from(liquidity) * Q64 + BigUint::from(input) * sqrt, true)?
            .to_u128().ok_or_else(|| error("sqrt overflow"))
    } else {
        sqrt.checked_add(mul_div(input, Q64, liquidity, false)?)
            .ok_or_else(|| error("sqrt overflow"))
    }
}
