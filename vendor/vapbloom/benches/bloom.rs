use criterion::{criterion_group, criterion_main, Criterion};
use vapbloom::{Bloom, Input};
use hex_literal::hex;
use tiny_keccak::keccak256;

fn test_bloom() -> Bloom {
	use std::str::FromStr;
	Bloom::from_str(
		"00000000000000000000000000000000\
		 00000000100000000000000000000000\
		 00000000000000000000000000000000\
		 00000000000000000000000000000000\
		 00000000000000000000000000000000\
		 00000000000000000000000000000000\
		 00000002020000000000000000000000\
		 00000000000000000000000800000000\
		 10000000000000000000000000000000\
		 00000000000000000000001000000000\
		 00000000000000000000000000000000\
		 00000000000000000000000000000000\
		 00000000000000000000000000000000\
		 00000000000000000000000000000000\
		 00000000000000000000000000000000\
		 00000000000000000000000000000000"
	).unwrap()
}

fn test_topic() -> Vec<u8> {
	hex!("02c69be41d0b7e40352fc85be1cd65eb03d40ef8427a0ca4596b1ead9a00e9fc").to_vec()
}

fn test_address() -> Vec<u8> {
	hex!("ef2d6d194084c2de36e0dabfce45d046b37d1106").to_vec()
}

fn test_dummy() -> Vec<u8> {
	b"123456".to_vec()
}

fn test_dummy2() -> Vec<u8> {
	b"654321".to_vec()
}

fn bench_accrue(c: &mut Criterion) {
	c.bench_function("accrue_raw", |b| {
		let mut bloom = Bloom::default();
		let topic = test_topic();
		let address = test_address();
		b.iter(|| {
			bloom.accrue(Input::Raw(&topic));
			bloom.accrue(Input::Raw(&address));
		})
	});
	c.bench_function("accrue_hash", |b| {
		let mut bloom = Bloom::default();
		let topic = keccak256(&test_topic());
		let address = keccak256(&test_address());
		b.iter(|| {
			bloom.accrue(Input::Hash(&topic));
			bloom.accrue(Input::Hash(&address));
		})
	});
}

fn bench_contains(c: &mut Criterion) {
	c.bench_function("contains_input_raw", |b| {
		let bloom = test_bloom();
		let topic = test_topic();
		let address = test_address();
		b.iter(|| {
			assert!(bloom.contains_input(Input::Raw(&topic)));
			assert!(bloom.contains_input(Input::Raw(&address)));
		})
	});
	c.bench_function("contains_input_hash", |b| {
		let bloom = test_bloom();
		let topic = keccak256(&test_topic());
		let address = keccak256(&test_address());
		b.iter(|| {
			assert!(bloom.contains_input(Input::Hash(&topic)));
			assert!(bloom.contains_input(Input::Hash(&address)));
		})
	});
}

fn bench_not_contains(c: &mut Criterion) {
	c.bench_function("does_not_contain_raw", |b| {
		let bloom = test_bloom();
		let dummy = test_dummy();
		let dummy2 = test_dummy2();
		b.iter(|| {
			assert!(!bloom.contains_input(Input::Raw(&dummy)));
			assert!(!bloom.contains_input(Input::Raw(&dummy2)));
		})
	});
	c.bench_function("does_not_contain_hash", |b| {
		let bloom = test_bloom();
		let dummy = keccak256(&test_dummy());
		let dummy2 = keccak256(&test_dummy2());
		b.iter(|| {
			assert!(!bloom.contains_input(Input::Hash(&dummy)));
			assert!(!bloom.contains_input(Input::Hash(&dummy2)));
		})
	});
	c.bench_function("does_not_contain_random_hash", |b| {
		let bloom = test_bloom();
		let dummy: Vec<_> = (0..255u8).map(|i| keccak256(&[i])).collect();
		b.iter(|| {
			for d in &dummy {
				assert!(!bloom.contains_input(Input::Hash(d)));
			}
		})
	});
}

criterion_group!(benches, bench_accrue, bench_contains, bench_not_contains);
criterion_main!(benches);
