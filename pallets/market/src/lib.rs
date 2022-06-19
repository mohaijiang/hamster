#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{dispatch::DispatchResult,
                    pallet_prelude::*, PalletId, traits::{Currency, ExistenceRequirement}};
use frame_support::sp_runtime::traits::Convert;
use frame_support::traits::UnixTime;
use frame_system::pallet_prelude::*;
use primitives::p_market;
use sp_core::Bytes;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::traits::Zero;
use sp_std::convert::TryInto;
use sp_std::vec::Vec;

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>

pub use pallet::*;
pub use primitives::p_provider::*;
pub use primitives::p_resource_order::*;
pub use primitives::p_market::*;
use primitives::EraIndex;

type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
const PALLET_ID: PalletId = PalletId(*b"ttchain!");

#[frame_support::pallet]
pub mod pallet {
    use frame_system::Origin;
    use primitives::p_market;

    use super::*;

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// currency to pay fees and hold balances
        type Currency: Currency<Self::AccountId>;

        /// order fee interface
        type OrderInterface: OrderInterface<AccountId=Self::AccountId, BlockNumber=Self::BlockNumber>;

        /// block height to number
        type BlockNumberToNumber: Convert<Self::BlockNumber, u128> + Convert<u32, Self::BlockNumber>;

        /// digital transfer amount
        type NumberToBalance: Convert<u128, BalanceOf<Self>>;
        /// amount converted to numbers
        type BalanceToNumber: Convert<BalanceOf<Self>, u128>;

        /// health check interval
        #[pallet::constant]
        type HealthCheckInterval: Get<Self::BlockNumber>;

        /// time
        type UnixTime: UnixTime;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(_);

    /// Store the pledge account number corresponding to the AccountId
    #[pallet::storage]
    #[pallet::getter(fn staking)]
    pub(super) type StakingAccontId<T: Config> = StorageMap<
        _,
        Twox64Concat,
        T::AccountId,
        p_market::StakingAmount,
        OptionQuery,
    >;

    /// Storage account revenue
    #[pallet::storage]
    #[pallet::getter(fn gateway_revenue)]
    pub(super) type GatewayRevenue<T: Config> = StorageMap<
        _,
        Twox64Concat,
        T::AccountId,
        Income,
        OptionQuery,
    >;

    /// Storage gateway points
    #[pallet::storage]
    #[pallet::getter(fn gateway_points)]
    pub(super) type GatewayPoints<T: Config> = StorageMap<
        _,
        Twox64Concat,
        T::AccountId,
        u128,
        OptionQuery,
    >;

    /// Current total amount in the staking_pot
    #[pallet::storage]
    #[pallet::getter(fn current_total_staking)]
    pub(super) type CurrentTotalStaking<T: Config> = StorageValue<_, u128, ValueQuery>;

    /// Current total amount in the market_reward_pot
    #[pallet::storage]
    #[pallet::getter(fn current_total_reward)]
    pub(super) type CurrentTotalReward<T: Config> = StorageValue<_, u128, ValueQuery>;

    /// storage gateway total points
    #[pallet::storage]
    #[pallet::getter(fn gateway_total_points)]
    pub(super) type GatewayTotalPoints<T: Config> = StorageValue<_, u128, ValueQuery>;

    /// 存储用户对应的金额
    /// Storage overdue proceeds
    #[pallet::storage]
    #[pallet::getter(fn overdue_proceeds)]
    pub(super) type OverdueProceeds<T: Config> = StorageValue<_, u128, ValueQuery>;

    // Pallets use events to inform users when important changes are made.
    // https://substrate.dev/docs/en/knowledgebase/runtime/events
    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId")]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        // Create of staking account successful
        CreateStakingAccountSuccessful(T::AccountId),

        // Staking account has exit 
        StakingAccountArealdyExit(T::AccountId),

        // Successful charge to staking account
        ChargeStakingAccountSuccessful(T::AccountId),

        // User success withdraw the price
        WithdrawStakingSuccess(T::AccountId, BalanceOf<T>),

        // Reward issued successfully
        RewardIssuedSucces(u128),

        // compute_gateways_rewards
        ComputeGatewaysRewardSuccess,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        // the staking accoutid is already exit in the market
        StakingAccontIdAlreadyExit,

        // the staking accoutid is not exit int the market
        StakingAccountIdNotExit,

        // the staking accoutid has not enough amount to Withdraw
        NotEnoughActiveAmount,

        // Users are not rewarded enough
        NotEnoughReward,
    }

    // Dispatchable functions allows users to interact with the pallet and invoke state changes.
    // These functions materialize as "extrinsics", which are often compared to transactions.
    // Dispatchable functions must be annotated with a weight and must return a DispatchResult.
    #[pallet::call]
    impl<T: Config> Pallet<T> {
       
        // 首先需要几个函数
        /*
        +amount:Balance 质押的金额
        +lockAmount:Balance 锁定的金额
        
        +stakingAmount(price) 质押金额
        +lockAmount() 锁定金额
        +unLockAmount() 解锁金额
        + withdrawAmount() 取回金额
        */
        
        // 为accountId绑定 stakingAmount， 即注册
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
        pub fn crate_staking_amount(
            origin: OriginFor<T>,
        ) ->DispatchResult {
            // mut be singed 
            let who = ensure_signed(origin)?;

            // Is already registered
            if StakingAccontId::<T>::contains_key(who.clone()) {
                return Err(Error::<T>::StakingAccontIdAlreadyExit.into());
            }

            // Binding stakingamount to Account
            StakingAccontId::<T>::insert(who.clone(), p_market::StakingAmount {
                amount: 0,
                active_amount: 0,
                lock_amount: 0,
            });

            Self::deposit_event(Event::CreateStakingAccountSuccessful(who.clone()));

            Ok(())
        }

        // charge for account 
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
        pub fn charge_for_account(
            origin: OriginFor<T>,
            bond_price: BalanceOf<T>,
        ) ->DispatchResult {

            let who = ensure_signed(origin)?;

            // Determine if a user exist staking account
            if !StakingAccontId::<T>::contains_key(who.clone()) {
                return Err(Error::<T>::StakingAccountIdNotExit.into());
            }

            // transfer accountid token to staking pot
            T::Currency::transfer(
                &who.clone(), 
                &Self::staking_pot(), 
                bond_price, 
                ExistenceRequirement::KeepAlive,
            )?;

            // Update the current total staking amount
            let mut staking = CurrentTotalStaking::<T>::get();
            staking += T::BalanceToNumber::convert(bond_price);
            CurrentTotalStaking::<T>::set(staking);

            // get pledge details
            let mut staking_info = StakingAccontId::<T>::get(who.clone()).unwrap();
            // calculate the new total pledge amount
            let price = T::BalanceToNumber::convert(bond_price);
            // charge for account
            staking_info.charge_for_account(price);
            // save the account amount 
            StakingAccontId::<T>::insert(
                who.clone(),
                staking_info,
            );

            Self::deposit_event(Event::ChargeStakingAccountSuccessful(who.clone()));

            Ok(())
        }

        // Withdraw amount from staking account 
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
        pub fn withdraw_amount(
            origin: OriginFor<T>,
            price: BalanceOf<T>,
        ) ->DispatchResult {

            let who = ensure_signed(origin)?;

            // Determine if a user exist staking account
            ensure!(
                StakingAccontId::<T>::contains_key(who.clone()),
                Error::<T>::StakingAccountIdNotExit,
            );

            // Get the staking info from user
            let mut staking_info = StakingAccontId::<T>::get(who.clone()).unwrap();

            // Update staking infformation
            // And determine if the user have enough active amount
            ensure!(
                staking_info.withdraw_amount(T::BalanceToNumber::convert(price)),
                Error::<T>::NotEnoughActiveAmount,
            );

            // Transfer the price from staking pot to user
            T::Currency::transfer(
                &Self::staking_pot(), 
                &who.clone(), 
                price, 
                ExistenceRequirement::AllowDeath,
            )?;

            StakingAccontId::<T>::insert(who.clone(), staking_info);

            // update the current total staking amount
            let mut totalstaking = CurrentTotalStaking::<T>::get();
            totalstaking -= T::BalanceToNumber::convert(price);
            CurrentTotalStaking::<T>::set(totalstaking);

            Self::deposit_event(Event::WithdrawStakingSuccess(who.clone(), price));
            Ok(())
        }
        
        /// payout
        /// Every user can run this function
        /// Get all the history reward to gateway whose has reward
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
        pub fn payout(
            origin: OriginFor<T>,
        ) ->DispatchResult {
            // Just check the signed
            ensure_signed(origin)?;

            // 轮询GatewayRevenue
            let gateway_nodes = GatewayRevenue::<T>::iter();

            let mut total_revenue = 0;

            for (accoutid, income) in gateway_nodes {
                // 从 storage 中 转income钱到 accountid 中
                T::Currency::transfer(
                    &Self::market_reward_pot(),
                    &accoutid, 
                    income.total_income.try_into().ok().unwrap(), 
                    ExistenceRequirement::KeepAlive,
                )?;
                // Update all the amount has payout this time
                total_revenue += income.total_income;

                // remove the revenue info
                GatewayRevenue::<T>::remove(accoutid.clone());
            }

            // Update the current total reward amount
            let mut current_total_reward = CurrentTotalReward::<T>::get();
            current_total_reward -= total_revenue;
            CurrentTotalReward::<T>::set(current_total_reward);

            // Send the amount which total payout this time 
            Self::deposit_event(Event::RewardIssuedSucces(total_revenue));
            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// StakingPod: use to storage the market people's stake amount 
    pub fn staking_pot() -> T::AccountId { PALLET_ID.into_sub_account(b"stak") }
    /// market_reward_pot: use to storage the market's reward from end_era
    pub fn market_reward_pot() -> T::AccountId { PALLET_ID.into_sub_account(b"reward") }

    // 将逾期未取的钱推送到国库里面
    // The function will transfer the overdue amount to the treasury
    // The The period is 60 Era
    // input:
    //  -index: EraIndex
    fn clearance_overdue_property(index: EraIndex) {
        let gateway_revenues = GatewayRevenue::<T>::iter();
        for (who, gateway_income) in gateway_revenues {
            if gateway_income.last_eraindex - index > 60 {
                // 删除该id的信息
                // 因为奖励时从staking 池子里面的发放的
                // 所以只要删除了信息，就相当于 把逾期的钱会回归给池子了
                GatewayRevenue::<T>::remove(who.clone());
                // 更新池子中，获得拿到逾期的钱
                let mut op = OverdueProceeds::<T>::get();
                op += gateway_income.total_income;
                OverdueProceeds::<T>::set(op);
            }
        }
    }
}

impl<T: Config> MarketInterface<<T as frame_system::Config>::AccountId> for Pallet<T> {
    
    fn storage_pot() -> <T as frame_system::Config>::AccountId {
        Self::storage_pot()
    }

    // Check the accountid have staking accoutid
    fn staking_accountid_exit(who: <T as frame_system::Config>::AccountId) -> bool {
        StakingAccontId::<T>::contains_key(who.clone())
    }

    // Return the staking info
    fn staking_info(who: <T as frame_system::Config>::AccountId) -> p_market::StakingAmount {
        StakingAccontId::<T>::get(who.clone()).unwrap().clone()
    }

    // updata staking info 
    fn updata_staking_info(who: <T as frame_system::Config>::AccountId, staking_info: p_market::StakingAmount) {
        StakingAccontId::<T>::insert(who.clone(), staking_info);
    }

    // The function score calculation by online time
    // score = blocknums * 10
    // input:
    //  - account: AccoutId
    //  - blocknums: u128
    fn compute_gateways_points(account: <T as frame_system::Config>::AccountId, blocknums: u128) {
        // 每在线一个区块 算10分
        let points = blocknums * 10;
        // 保存得分
        if GatewayPoints::<T>::contains_key(account.clone()) {
            let mut _points = GatewayPoints::<T>::get(account.clone()).unwrap();
            _points = points;
            GatewayPoints::<T>::insert(account.clone(), points);
            return;
        }
        // 不存在 直接插入
        GatewayPoints::<T>::insert(account.clone(), points);
    }


    // Todo 
    // 检查有无逾期未取的钱
    // 计算奖励金额，将奖励金额更新到用户上面
    // 通过这个时期在线的gateway的在线时长去计算
    // input：
    //  - index： EraIndex
    fn compute_gateways_rewards(index: EraIndex, total_reward: u128) {
        
        // 先检查一遍是否有过期未取的钱
        Self::clearance_overdue_property(index);
        // 计算每个gateway获得的分数占比
        let gateway_points = GatewayPoints::<T>::iter();

        for (who, point) in gateway_points {
            // 计算该分数的占比
            let ratio = point / GatewayTotalPoints::<T>::get();
            // 该账号获得的奖励
            let reward = total_reward * ratio;

            // 将奖励计算到 账户上
            // 判断 奖励账号是否存在该账号
            if GatewayRevenue::<T>::contains_key(who.clone()) {
                // 存在该账户，修改income信息
                let mut income = GatewayRevenue::<T>::get(who.clone()).unwrap();
                income.total_income += reward;
                GatewayRevenue::<T>::insert(who.clone(), income);
                // 进入下一轮循环
                continue;
            }

            // create the income struct 
            let income = Income {
                last_eraindex: index,
                total_income: reward,
            };

            GatewayRevenue::<T>::insert(who.clone(), income);
        }

        // Update the current total reward
        let mut _total_reward = CurrentTotalReward::<T>::get();
        _total_reward += total_reward;
        CurrentTotalReward::<T>::set(_total_reward);

        Self::deposit_event(Event::ComputeGatewaysRewardSuccess);
    }

    
}