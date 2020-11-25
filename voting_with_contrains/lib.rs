#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

#[ink::contract]
mod voting {
	use ink_prelude::vec::Vec;
	use ink_storage::{
		collections::{HashMap as StorageHashMap, Vec as StorageVec},
		traits::{PackedLayout, SpreadLayout},
	};

	#[derive(scale::Encode, scale::Decode)]
	#[cfg_attr(
		feature = "std",
		derive(scale_info::TypeInfo, Debug, SpreadLayout, PackedLayout, PartialEq, Eq,)
	)]
	pub struct VoteOfCandidate {
		candidate: AccountId,
		vote: u32,
	}
	// 定义持久化变量
	// votes_received: 每个用户获取的投票数量
	// candidate_list: 可被投票的用户列表
	// in_candidate_list: 冗余信息用于快速判断某个用户是否在可投票列表中
	// total balance: 总的票数上限
	// balance_token: 剩余票数
	// token_price: 每张票的价格
	// vote_num: 谁投了谁几票
	// voter_balance: 投票人买了几张票
	#[ink(storage)]
	pub struct Voting {
		votes_received: StorageHashMap<AccountId, u32>,
		candidate_list: StorageVec<AccountId>,
		in_candidate_list: StorageHashMap<AccountId, ()>,
		total_tokens: u32,
		balance_tokens: u32,
		token_price: u32,
		vote_num: StorageHashMap<(AccountId, AccountId), u32>,
		voter_balance: StorageHashMap<AccountId, u32>,
	}

	// 投票触发事件定义
	#[ink(event)]
	pub struct VoteEvent {
		#[ink(topic)]
		from: AccountId,
		#[ink(topic)]
		to: AccountId,
	}

	impl Voting {
		#[ink(constructor)]
		pub fn new(lists: Vec<AccountId>, total_tokens: u32, token_price: u32) -> Self {
			let in_candidate_list: StorageHashMap<_, _, _> =
				lists.iter().copied().map(|x| (x, ())).collect();
			let candidate_list: StorageVec<_> = lists.iter().copied().collect();
			assert!(in_candidate_list.len() == candidate_list.len());
			Self {
				candidate_list,
				votes_received: StorageHashMap::default(),
				in_candidate_list,
				total_tokens,
				balance_tokens: total_tokens,
				token_price,
				vote_num: StorageHashMap::default(),
				voter_balance: StorageHashMap::default(),
			}
		}

		#[ink(message)]
		pub fn buy_ticket(&mut self, owner: AccountId, value: u32) -> bool {
			let amount = value / self.token_price;
			// 确保剩余票数够
			if amount > self.balance_tokens {
				return false;
			}
			// 用户ticket增加
			if !self.voter_balance.contains_key(&owner) {
				// 新用户直接插入数据
				self.voter_balance.insert(owner, amount);
			} else {
				// 老用户做mutate
				self.voter_balance.entry(owner).and_modify(|v| *v += amount);
			}

			// balance_tokens减少
			self.balance_tokens -= amount;

			true
		}
		// 剩余票数
		#[ink(message)]
		pub fn all_ticket_num(&mut self) -> u32 {
			self.total_tokens
		}
		// 剩余票数
		#[ink(message)]
		pub fn left_ticket_num(&mut self) -> u32 {
			self.balance_tokens
		}
		// 购买一票需要的价格
		#[ink(message)]
		pub fn price_of_ticket(&mut self) -> u32 {
			self.token_price
		}
		// 某用户手中的票数
		#[ink(message)]
		pub fn voter_ticket_balance(&mut self, owner: AccountId) -> u32 {
			*self.voter_balance.get(&owner).unwrap_or(&0)
		}
		// 获取可被投票的用户数量
		#[ink(message)]
		pub fn get_candidates_len(&mut self) -> u32 {
			self.candidate_list.len()
		}
		// 获取可被投票的用户
		#[ink(message)]
		pub fn get_candidates(&mut self) -> Vec<AccountId> {
			self.candidate_list.iter().copied().collect()
		}
		// 获取当前各用户投票票数状态
		#[ink(message)]
		pub fn get_current_votes(&mut self) -> Vec<VoteOfCandidate> {
			let candidate_list: Vec<_> = self.candidate_list.iter().copied().collect();
			let mut current_vote: Vec<VoteOfCandidate> = Vec::new();
			for x in candidate_list.clone().into_iter() {
				let candidate = VoteOfCandidate {
					candidate: x,
					vote: self.my_value_or_zero(x),
				};
				current_vote.push(candidate);
			}
			current_vote
		}

		// 投票
		// owner 投票人
		// candidate 被投票人
		// amout 投票数量
		#[ink(message)]
		pub fn vote_candidate(&mut self, owner: AccountId, candidate: AccountId, amout: u32) -> bool {
			let ret: bool = self.vote_candidate_without_event(owner, candidate, amout);
			if ret {
				self.env().emit_event(VoteEvent {
					from: self.env().caller(),
					to: candidate,
				});
			}
			ret
		}

		// it seems unit test failed when emit event if call vote_candidate function directly
		fn vote_candidate_without_event(
			&mut self,
			owner: AccountId,
			candidate: AccountId,
			amout: u32,
		) -> bool {
			// 1. 首先确认被投票人在candidate_list中
			if !self.in_candidate_list.contains_key(&candidate) {
				return false;
			}
			// 2. 确认投票人有足够的票数
			let ticket_num = self.voter_ticket_balance(owner);
			if ticket_num < amout {
				return false;
			}

			// 3. 投票者票数减少
			self.voter_balance.entry(owner).and_modify(|v| *v -= amout);
			// 4. 更新voter
			self
				.vote_num
				.entry((owner, candidate))
				.and_modify(|v| *v += amout)
				.or_insert(amout);
			// 5. 候选人票数增加
			self
				.votes_received
				.entry(candidate)
				.and_modify(|v| *v += amout)
				.or_insert(amout);
			true
		}

		// 获取某用户被投票的数量
		#[ink(message)]
		pub fn total_votes_for(&self, candidate: AccountId) -> u32 {
			self.my_value_or_zero(candidate)
		}

		// 获取某用户被投票的数量
		#[ink(message)]
		pub fn callee_vote_of(&self, callee: AccountId, candidate: AccountId) -> u32 {
			*self.vote_num.get(&(callee, candidate)).unwrap_or(&0)
		}

		// 内部辅助函数用户确认某用户是否被存在candidate_list中
		fn valid_candidate(&self, candidate: AccountId) -> bool {
			for x in self.candidate_list.into_iter() {
				if *x == candidate {
					return true;
				}
			}
			return false;
		}

		// 内部辅助函数用户获取某用户的投票数量
		fn my_value_or_zero(&self, of: AccountId) -> u32 {
			let value = self.votes_received.get(&of).unwrap_or(&0);
			*value
		}
	}

	#[cfg(test)]
	mod tests {
		use super::*;
		use ink_env::test;
		use ink_prelude::vec::Vec;
		use ink_storage::collections::Vec as StorageVec;
		type Accounts = test::DefaultAccounts<Environment>;
		fn default_accounts() -> Accounts {
			test::default_accounts().expect("Test environment is expected to be initialized.")
		}
		#[test]
		fn default_works() {
			let mut voting = Voting::new(Vec::new(), 100, 1);
			assert_eq!(voting.candidate_list, StorageVec::new());
			assert_eq!(voting.candidate_list.len(), 0);
			assert_eq!(voting.get_candidates_len(), 0);
			assert_eq!(voting.all_ticket_num(), 100);
			assert_eq!(voting.left_ticket_num(), 100);
			assert_eq!(voting.price_of_ticket(), 1);
		}

		#[test]
		fn init_candidates() {
			let accounts = default_accounts();
			let candidates = ink_prelude::vec![accounts.alice, accounts.bob, accounts.eve];
			let mut voting = Voting::new(candidates, 100, 1);
			assert_eq!(voting.candidate_list.len(), 3);
			assert_eq!(voting.all_ticket_num(), 100);
			assert_eq!(voting.left_ticket_num(), 100);
			assert_eq!(voting.price_of_ticket(), 1);
		}

		#[test]
		fn buy_works() {
			let accounts = default_accounts();
			let candidates = ink_prelude::vec![accounts.alice, accounts.bob, accounts.eve];
			let mut voting = Voting::new(candidates, 100, 1);
			assert_eq!(voting.voter_ticket_balance(accounts.alice), 0);
			assert_eq!(voting.left_ticket_num(), 100);
			assert_eq!(voting.buy_ticket(accounts.alice, 1), true);
			assert_eq!(voting.left_ticket_num(), 99);
			assert_eq!(voting.voter_ticket_balance(accounts.alice), 1);
			assert_eq!(voting.buy_ticket(accounts.alice, 1), true);
			assert_eq!(voting.left_ticket_num(), 98);
			assert_eq!(voting.voter_ticket_balance(accounts.alice), 2);
			assert_eq!(voting.buy_ticket(accounts.bob, 1), true);
			assert_eq!(voting.left_ticket_num(), 97);
			assert_eq!(voting.voter_ticket_balance(accounts.alice), 2);
			assert_eq!(voting.voter_ticket_balance(accounts.bob), 1);
		}

		#[test]
		fn voter_balance_work() {
			let accounts = default_accounts();
			let candidates = ink_prelude::vec![accounts.alice, accounts.bob, accounts.eve];
			let mut voting = Voting::new(candidates, 100, 1);
			assert_eq!(voting.voter_ticket_balance(accounts.alice), 0);
			assert_eq!(voting.buy_ticket(accounts.alice, 10), true);
			assert_eq!(voting.voter_ticket_balance(accounts.alice), 10);
		}

		#[test]
		fn vote_works() {
			let accounts = default_accounts();
			let candidates = ink_prelude::vec![accounts.alice, accounts.bob, accounts.eve];
			let mut voting = Voting::new(candidates, 100, 1);
			assert_eq!(voting.buy_ticket(accounts.alice, 10), true);
			assert_eq!(voting.voter_ticket_balance(accounts.alice), 10);
			assert_eq!(voting.total_votes_for(accounts.bob), 0);
			assert_eq!(
				voting.vote_candidate_without_event(accounts.alice, accounts.bob, 1),
				true
			);
			assert_eq!(voting.voter_ticket_balance(accounts.alice), 9);
			assert_eq!(voting.total_votes_for(accounts.bob), 1);
			assert_eq!(voting.callee_vote_of(accounts.alice, accounts.bob), 1);
		}

		#[test]
		fn vote_invalid_candidate_does_not_work() {
			let accounts = default_accounts();
			let candidates = ink_prelude::vec![accounts.alice, accounts.bob];
			let mut voting = Voting::new(candidates, 100, 1);
			assert_eq!(voting.buy_ticket(accounts.alice, 10), true);
			assert_eq!(
				voting.vote_candidate_without_event(accounts.alice, accounts.eve, 1),
				false
			);
		}

		#[test]
		fn ticket_not_enough_does_not_work() {
			let accounts = default_accounts();
			let candidates = ink_prelude::vec![accounts.alice, accounts.bob];
			let mut voting = Voting::new(candidates, 100, 1);
			assert_eq!(voting.buy_ticket(accounts.alice, 1), true);
			assert_eq!(
				voting.vote_candidate_without_event(accounts.alice, accounts.bob, 2),
				false
			);
		}

		#[test]
		fn anyone_could_buy_ticket() {
			let accounts = default_accounts();
			let candidates = ink_prelude::vec![accounts.alice, accounts.bob];
			let mut voting = Voting::new(candidates, 100, 1);
			assert_eq!(voting.buy_ticket(accounts.eve, 10), true);
		}

		#[test]
		fn get_current_votes_works() {
			let accounts = default_accounts();
			let candidates = ink_prelude::vec![accounts.alice, accounts.bob, accounts.eve];
			let mut voting = Voting::new(candidates.clone(), 100, 1);
			let current = voting.get_current_votes();
			assert_eq!(current.len(), 3);
			assert_eq!(current[0].vote, 0);
			assert_eq!(current[1].vote, 0);
			assert_eq!(current[2].vote, 0);
			assert_eq!(voting.buy_ticket(accounts.alice, 10), true);
			voting.vote_candidate_without_event(accounts.alice, accounts.alice, 1);
			let current = voting.get_current_votes();
			assert_eq!(current.len(), 3);
			assert_eq!(current[0].vote, 1);
			assert_eq!(current[1].vote, 0);
			assert_eq!(current[2].vote, 0);
		}
	}
}
