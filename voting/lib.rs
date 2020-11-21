#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

#[ink::contract]
mod voting {
	use ink_prelude::vec::Vec;
	use ink_storage::collections::{HashMap as StorageHashMap, Vec as StorageVec};
	// 定义持久化变量
	// votes_received: 每个用户获取的投票数量
	// candidate_list: 可被投票的用户列表
	// in_candidate_list: 冗余信息用于快速判断某个用户是否在可投票列表中
	#[ink(storage)]
	pub struct Voting {
		votes_received: StorageHashMap<AccountId, u32>,
		candidate_list: StorageVec<AccountId>,
		in_candidate_list: StorageHashMap<AccountId, ()>,
	}

	#[derive(scale::Encode, scale::Decode)]
	#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
	pub struct CurrentVote {
		candidate_list: Vec<AccountId>,
		current_vote: Vec<u32>,
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
		pub fn new(lists: Vec<AccountId>) -> Self {
			let in_candidate_list: StorageHashMap<_, _, _> =
				lists.iter().copied().map(|x| (x, ())).collect();
			let candidate_list: StorageVec<_> = lists.iter().copied().collect();
			assert!(in_candidate_list.len() == candidate_list.len());
			Self {
				candidate_list,
				votes_received: StorageHashMap::default(),
				in_candidate_list,
			}
		}

		// 获取可被投票的用户数量
		#[ink(message)]
		pub fn get_candidates_len(&mut self) -> u32 {
			self.candidate_list.len()
		}

		// 获取可被投票的用户
		#[ink(message)]
		pub fn get_candidates(&mut self) -> Vec<AccountId> {
			let lists: Vec<_> = self.candidate_list.iter().copied().collect();
			lists
		}

		// 获取当前各用户投票票数状态
		#[ink(message)]
		pub fn get_current_votes(&mut self) -> CurrentVote {
			let candidate_list: Vec<_> = self.candidate_list.iter().copied().collect();
			let mut current_vote: Vec<u32> = Vec::new();
			for x in candidate_list.clone().into_iter() {
				current_vote.push(self.my_value_or_zero(x));
			}
			CurrentVote {
				candidate_list,
				current_vote,
			}
		}

		// 投票
		#[ink(message)]
		pub fn vote_candidate(&mut self, candidate: AccountId) -> bool {
			let ret: bool = self.vote_candidate_without_event(candidate);
			if ret {
				self.env().emit_event(VoteEvent {
					from: self.env().caller(),
					to: candidate,
				});
			}
			ret
		}

		// it seems unit test failed when emit event if call vote_candidate function directly
		fn vote_candidate_without_event(&mut self, candidate: AccountId) -> bool {
			if !self.in_candidate_list.contains_key(&candidate) {
				return false;
			}
			self
				.votes_received
				.entry(candidate)
				.and_modify(|v| *v += 1)
				.or_insert(1);
			true
		}

		// 获取某用户被投票的数量
		#[ink(message)]
		pub fn total_votes_for(&self, candidate: AccountId) -> u32 {
			self.my_value_or_zero(candidate)
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
			let mut voting = Voting::new(Vec::new());
			assert_eq!(voting.candidate_list, StorageVec::new());
			assert_eq!(voting.candidate_list.len(), 0);
			assert_eq!(voting.get_candidates_len(), 0);
		}

		#[test]
		fn init_candidates() {
			let accounts = default_accounts();
			let candidates = ink_prelude::vec![accounts.alice, accounts.bob, accounts.eve];
			let voting = Voting::new(candidates);
			assert_eq!(voting.candidate_list.len(), 3);
		}

		#[test]
		fn vote_works() {
			let accounts = default_accounts();
			let candidates = ink_prelude::vec![accounts.alice, accounts.bob, accounts.eve];
			let mut voting = Voting::new(candidates);
			assert_eq!(voting.total_votes_for(accounts.alice), 0);
			assert_eq!(voting.total_votes_for(accounts.bob), 0);
			assert_eq!(voting.total_votes_for(accounts.eve), 0);
			voting.vote_candidate_without_event(accounts.alice);
			assert_eq!(voting.total_votes_for(accounts.alice), 1);
			voting.vote_candidate_without_event(accounts.alice);
			assert_eq!(voting.total_votes_for(accounts.alice), 2);
			assert_eq!(voting.total_votes_for(accounts.bob), 0);
			assert_eq!(voting.total_votes_for(accounts.eve), 0);
			voting.vote_candidate_without_event(accounts.bob);
			assert_eq!(voting.total_votes_for(accounts.alice), 2);
			assert_eq!(voting.total_votes_for(accounts.bob), 1);
			assert_eq!(voting.total_votes_for(accounts.eve), 0);
		}

		#[test]
		fn vote_invalid_candidate_does_not_work() {
			let accounts = default_accounts();
			let candidates = ink_prelude::vec![accounts.alice, accounts.bob];
			let mut voting = Voting::new(candidates);
			assert_eq!(voting.total_votes_for(accounts.alice), 0);
			assert_eq!(voting.total_votes_for(accounts.bob), 0);
			assert_eq!(voting.vote_candidate(accounts.eve), false);
		}

		#[test]
		fn get_current_votes_works() {
			let accounts = default_accounts();
			let candidates = ink_prelude::vec![accounts.alice, accounts.bob, accounts.eve];
			let mut voting = Voting::new(candidates.clone());
			let current = voting.get_current_votes();
			assert_eq!(current.candidate_list, candidates.clone());
			assert_eq!(current.current_vote, ink_prelude::vec![0, 0, 0]);
			voting.vote_candidate_without_event(accounts.alice);
			let current = voting.get_current_votes();
			assert_eq!(current.candidate_list, candidates.clone());
			assert_eq!(current.current_vote, ink_prelude::vec![1, 0, 0]);
		}
	}
}
