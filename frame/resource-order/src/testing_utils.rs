use crate::Pallet as ResourceOrder;
use crate::*;
use frame_benchmarking::account;
use frame_system::RawOrigin;

const SEED: u32 = 0;

/// Grab a funded user
pub fn create_funded_user<T: Config> (
	string: &'static str,
	n: u32,
	balance_factor: u32,
) -> T::AccountId {
	let user = account(string, n, SEED);
	let balance = T::Currency::minimum_balance() * balance_factor.into();
	T::Currency::make_free_balance_be(&user, balance.clone());
	T::Currency::issue(balance);

	user
}

/// Create an staking account
pub fn create_staking_account<T: Config> (
	n: u32,
	balance_factor: u32,
) -> T::AccountId {
	let user = create_funded_user::<T>("user", n, balance_factor);
	ResourceOrder::<T>::change_staking_for_benchmarking(user.clone());
	user
}

pub fn create_provider_resource<T: Config>(
	n: u32,
	balance_factor: u32,
) -> T::AccountId {
	let user = create_staking_account::<T>(n, balance_factor);
	T::ProviderInterface::create_resource_by_benchmarking(user.clone());
	user
}


