use num_traits::pow;
use std::collections::HashMap;
use rayon::prelude::*;

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{env,near_bindgen, AccountId, Balance, Duration, Timestamp, Promise, PromiseResult, Gas};
use near_sdk::serde::{Serialize, Deserialize};
use near_sdk::collections::{LookupMap, UnorderedMap};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

const MAX_TITLE_SIZE: usize = 20;
const MAX_METADATA_SIZE: usize = 150;
const RESERVED_FUNDS: Balance = 100_000_000_000_000_000_000_000;  //0,1Ⓝ
const CREATOR_BOND: Balance = 10_000_000_000_000_000_000_000_000; //10Ⓝ

type ProposalId = String;

#[allow(dead_code)]
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
//Proposal. Every registered user can create this contest. Other users(choicers) can submit their decisions and 
//  when voting starts they can vote for them. When election stage will be finished they get raised proposal funds
pub struct Proposal {
    status : ProposalStatus,
    vote_type : VoteType,
    id : ProposalId,
    title : String,
    funds : Balance,                 //Ⓝ attached to proposal
    owner : AccountId,
    metadata : String,               //description details
    max_decisions : u16,
    decisions : Vec<Decision>,
    vote_results : Vec<Votes>,
    start_time: Timestamp,
    //proposal_duration : Duration,  //TODO: proposal creator can set estimated time values for submitting decisions 
    //vote_duration : Duration,      //TODO: proposal creator can set estimated time values for submittting votes       
}
#[derive(BorshDeserialize, BorshSerialize, Debug, Clone)]
pub struct Decision {
    performer : AccountId,
    metadata : String
}
//It's not like (Yes/No) votes. You must ordering users from best to worst like:
//  1st place  - "account_1.near"
//  2nd place  - "account_2.near"
//  ...
//  last place - "account_last.near"
#[derive(BorshDeserialize, BorshSerialize, Debug, Serialize, Deserialize, Clone)]
pub struct Votes {
    from: AccountId,
    vote: HashMap<String, f64>
}
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(crate="near_sdk::serde")]
#[serde(tag="type")]
pub enum ProposalStatus {
    Open,
    Vote,
    Payout
}
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Debug, Clone)]
#[serde(crate="near_sdk::serde")]
#[serde(tag="type")]
//TODO: proposals will be not only for contests votes. DAO's can made Project Election vote, which allows
//      disperse pool funds for investments proportionally election decisions 
pub enum VoteType {
    PerformerElection,
    ProjectElection
}
#[derive(BorshDeserialize, BorshSerialize, Debug)]
//choicer - standart member of application
pub struct Choicer {
    account_id: AccountId,
    total_received: Balance,     //Ⓝ received as voter
    completed_choices: u16,
    current_choices: u16,
    proposals_created: u16,
    total_spending: Balance      //Ⓝ spended as proposal creator
}
//Accumulate votes and multisend proposal funds to every submitted participant proportionally to votes for them
#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq, Clone)]
struct VoteEngine {
    weights:Vec<f64>,
    results:Vec<HashMap<String, f64>>,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Contract {
    proposals : UnorderedMap<ProposalId, Proposal>,      
    choicers   : LookupMap<AccountId, Choicer>,
    vote_engine : VoteEngine                             
}
//impl Default panic -todo

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new() -> Self {
        assert!(
            !env::state_exists(),
            "The contract is already initialized",
        );

        Self {
            proposals: UnorderedMap::new(b"proposals".to_vec()),
            choicers: LookupMap::new(b"choicers".to_vec()),
            vote_engine : VoteEngine { 
                weights: Vec::new(),
                results: Vec::new()
            }
        }
    }
    //CREATOR SIDE
    #[payable]
    pub fn create_proposal(
        &mut self,
        vote_type : VoteType,
        title : String,
        funds : Balance,
        max_decisions : u16,
        //proposal_duration : Duration,
        //vote_duration : Duration, 
        metadata : String,
	) {

        let predecessor = env::predecessor_account_id();
        assert!(
            self.choicers.contains_key(&predecessor),
            "You must create a membership first to push your proposals"
        );

        let deposit = env::attached_deposit();
        assert!(
            deposit >= CREATOR_BOND,
            "You need have at least 10Ⓝ to create proposal" 
        );

        assert!(funds >= CREATOR_BOND, "Min deposit for proposal = {}Ⓝ", CREATOR_BOND);
        assert!(
            deposit > funds,
            "You need fill your balance on {}Ⓝ 
            to create proposal with funds value = {}Ⓝ", yton(funds - deposit), yton(funds) 
        );

        assert!(
            title.len() <= MAX_TITLE_SIZE && metadata.len() <= MAX_METADATA_SIZE ,
            "Too many symbols. Max title size is {} . Max description metadata size is {}", MAX_TITLE_SIZE, MAX_METADATA_SIZE
        );

        let performer = env::predecessor_account_id();
        //let proposal_id = bs58::encode(env::sha256(&env::random_seed())).into_string(); 
        let proposal_id = performer.to_string()+"001";                //for tests
        let proposal = Proposal {
            status : ProposalStatus::Open,
            vote_type,
            id : proposal_id.clone(),
            title,
            funds,
            owner : performer,
            metadata,
            max_decisions,
            decisions : Vec::new(),
            vote_results : Vec::new(),
            start_time: env::block_timestamp(),
            //proposal_duration,
            //vote_duration            
        };
	
        env::log_str(&(format!("Wow! Created a new Proposal: id {} proposal {:#?}", &proposal_id, &proposal).to_string()));
		
        let mut choicer = self.choicers
                .get(&predecessor)
                .expect(&(format!("No choicer with id @{}",predecessor)));

        choicer.current_choices += 1;
        choicer.proposals_created += 1;
        self.choicers.insert(&predecessor,&choicer);

        self.proposals.insert(&proposal_id, &proposal);
    }

    pub fn view_decisions(
        &self,
        proposal_id: String
    ) -> Vec<Decision> {
        self.proposals
            .get(&proposal_id)
            .expect(&(format!("No proposal with id {}",&proposal_id)))
            .decisions
    }

    #[payable]
    pub fn change_funds(
        &mut self,
        proposal_id: String,
        new_funds: f64
    ) {
        let mut proposal = self.proposals
            .get(&proposal_id)
            .expect(&(format!("No proposal with that id {}",proposal_id)));

        assert!(proposal.status == ProposalStatus::Open, "Proposal must be in Open status for changing funds");

        let owner = env::predecessor_account_id();
        assert!(
            proposal.owner == owner,
            "Only proposal creator can change funds"
        );
        let old_funds = proposal.funds;
        proposal.funds = ntoy(new_funds as Balance);

        self.proposals.insert(&proposal_id,&proposal);
        env::log_str(&(format!("Change funds from {}Ⓝ into {}Ⓝ  for proposal: {} ", yton(old_funds), yton(proposal.funds), proposal.title)));
    }
    //TODO: Made it actions when time = estimated_time for it (via Duration) 
    fn start_election(&mut self, proposal_id: String) {
        let mut proposal = self.proposals
            .get(&proposal_id)
            .expect(&(format!("No proposal with that id {}",&proposal_id)));
        proposal.status = ProposalStatus::Vote;

        self.proposals.insert(&proposal_id,&proposal);
    }
    fn finish_election(&mut self, proposal_id: String) {
        let mut proposal = self.proposals
            .get(&proposal_id)
            .expect(&(format!("No proposal with that id {}",&proposal_id)));
        proposal.status = ProposalStatus::Payout;

        self.proposals.insert(&proposal_id,&proposal);
    } 

//CHOICER SIDE
    #[payable]
    pub fn create_membership(&mut self) {
        assert!(
            env::attached_deposit() >= RESERVED_FUNDS,
            "You need have at least 0.1Ⓝ to create membership"
        );

        let member_id = env::predecessor_account_id();

        let member = Choicer {
            account_id:member_id.clone(),
            total_received:0,
            completed_choices:0,
            current_choices:0,
            proposals_created: 0,
            total_spending: 0 
        };

        env::log_str(&(format!("Choicer membership created. info: {:#?}", member)));

        self.choicers.insert(&member_id, &member);
    }
    //view all proposals with "Open" status
    fn is_active_proposal(&self, proposal_id: String) -> bool {
        let status = self.proposals
            .get(&proposal_id)
            .expect(&(format!("No proposal with id {}",proposal_id)))
            .status;
        status == ProposalStatus::Open
    }
    pub fn view_active_proposals(
        &self,
    ) -> UnorderedMap<ProposalId,Proposal> {
        let proposals = &self.proposals;
        let mut active_proposals: UnorderedMap<String, Proposal> = UnorderedMap::new(b"active".to_vec());
        for proposal in proposals.values() {
            if self.is_active_proposal(proposal.id.clone()) == true {
                active_proposals.insert(&proposal.id, &proposal);
            }
        }
        active_proposals
    } 
    pub fn is_a_member(&self, id: AccountId) -> bool {
        match self.choicers.get(&id) {
            Some(_v) => true,
            None => false
        }
    }
	//TODO: Add checking "is_proposal_owner?". Cause creator cannot vote/submit for their proposals
    pub fn submit_decision(&mut self, proposal_id: String, metadata: String) {
        let predecessor = env::predecessor_account_id();
        assert!(self.is_a_member(predecessor.clone()),"You are not member. Create membership first for submit decisions");
        
        let mut proposal = self.proposals
            .get(&proposal_id)
            .expect(&(format!("No proposal with id {}",&proposal_id)));
	    assert!(proposal.status == ProposalStatus::Open, "Proposal status is {:?} ", proposal.status);

        let decision = Decision {
            performer : predecessor,
            metadata
        };

        let member_id = env::predecessor_account_id();
        let mut choicer = self.choicers
            .get(&member_id)
            .expect(&(format!("No choicer with id @{}",member_id)));

        choicer.current_choices += 1;

        self.choicers.insert(&member_id,&choicer);
        env::log_str(&(format!(
           "Choicer @{} info: {:#?} 
            submitted decision {:?} for proposal {} ",
            member_id, choicer, decision, proposal.title)));

        proposal.decisions.push(decision);
		
        self.proposals.insert(&proposal_id, &proposal);
    }

    #[payable]
    //send your ranged and ordering votes for decisions. TODO: checking for predecessor submitted decision in proposal
    pub fn vote(&mut self, proposal_id: String, vote: HashMap<String, f64>) { 
        let member_id = env::predecessor_account_id();
        assert!(self.is_a_member(member_id.clone()),"You are not member. Create membership via same name function");
        
        let mut proposal = self.proposals
            .get(&proposal_id)
            .expect(&(format!("No proposal with that id {}",&proposal_id)));
        assert!(proposal.status == ProposalStatus::Vote, "Election is not started. Now proposal is still open");

        let choice = Votes {
            from: member_id,
            vote
        };

        proposal.vote_results.push(choice);

        self.proposals.insert(&proposal_id, &proposal);
    }
    //see all votes from choicers before final counted. status: Vote
    pub fn view_vote_board(&self, proposal_id: String ) -> Vec<Votes> { 
        let proposal = self.proposals
            .get(&proposal_id)
            .expect(&(format!("No proposal with that title {}",&proposal_id)));
        assert!(proposal.status == ProposalStatus::Vote, "Election not started. Now proposal is still open");
        proposal.vote_results
    } 
//---------------------------------------------------------
    //VOTE ENGINE
    // Calculate weights for vote table places. 
    //  Depends on number of participants (p) and proposal funds for disperse
    fn set_weights(&self, p: usize, funds: f64) -> Vec<f64> {
        let p:i32 = p as i32;
        let f:f64 = funds;
	    
        let aw:usize = (p-1) as usize;
        let w_last = f/(p*(pow(2, aw) - 1)) as f64;
		
        let mut vec: Vec<f64> = Vec::with_capacity(aw);
        let mut rev_vec: Vec<f64> = Vec::with_capacity(aw);
		
        // These are all done without reallocating...
        for i in 0..aw {
            vec.push(pow(2, i) as f64 * w_last.clone())
        }	
        for i in vec.iter().rev() {
            rev_vec.push(*i)
        }
        rev_vec
    }
    //Accumulate all votes and calculate values for multisend proposal funds
    fn calculate_vote_results(&mut self, proposal_id: String) -> HashMap<String, f64> {

        let proposal = self.proposals
            .get(&proposal_id)
            .expect(&(format!("No proposal with id {}",proposal_id))); 
        assert!(proposal.status == ProposalStatus::Payout, "Election not finished. Now choicers are still voting");
       
        let p = proposal.vote_results.len();
        let funds = proposal.funds.clone();
        let converted_funds = yton(funds) as f64;
        //set weights for calculate
        self.vote_engine.weights = self.set_weights(p, converted_funds);
        
        //Get vote results from contract
        let votes = proposal.vote_results;
        let mut results:Vec<HashMap<String, f64>> = Vec::new();
        for i in votes.into_iter() {
            results.push(i.vote)
        }
        //Convert every vote from vote results
        for mut i in results.into_iter() {
            //participant place in votes changes ( x => weights[x-1] )
            for item in i.values_mut() {
                *item = self.vote_engine.weights[(*item-1.0) as usize];
            }
            //push converted(weighted) results back
            self.vote_engine.results.push(i)
        }
        
        //accumulate all results in one instruction for multisend
        let v = self.vote_engine.results.clone();
        let bomb = v.into_par_iter()
            .fold(||HashMap::new(), |mut a: HashMap<String, f64>, b| {
                a.extend(b);
                a
            }).reduce(||HashMap::new(),|mut a, b| {
            for (k, v) in b {
                if a.contains_key(&k) {
                    let x = a.get(&k).unwrap();
                    a.insert(k, v + x);
                } else {
                    a.insert(k, v);
                }
            }
		    a
        });
        bomb
    }

    //Payout. Ⓝ mutisender based on vote results
    fn payout(&mut self, proposal_id: String) {
        let proposal = self.proposals
            .get(&proposal_id)
            .expect(&(format!("No proposal with id {}",proposal_id))); 
        
        assert!(proposal.status == ProposalStatus::Payout, "Election not finished. Now choicers are still voting");

        let votes = self.calculate_vote_results(proposal_id);
        let predecessor = env::predecessor_account_id();
        let deposit: Balance = proposal.funds;

        let mut total: f64 = 0.0;
        //Check accounts from votes and calculate sending total
        for account in votes.keys() {
            assert!(
                env::is_valid_account_id(predecessor.as_bytes()),
                "Account @{} is invalid",
                predecessor
            );

            let amount = votes[account].round();
            total += amount;
        }

        env::log_str(format!("Sending {}Ⓝ", total).as_str());

		let total_spending = total as Balance;
		assert!(
            total_spending <= yton(deposit),
            "Not enough attached tokens to run multisender (Supplied: {}Ⓝ. Demand: {}Ⓝ)",
            yton(deposit),
            total_spending
        );
        //Send Ⓝ proportionally vote results
        for vote in votes {
            let (account_id,amount) = vote;
            let amount_u128:Balance = (amount.round()) as Balance;

			env::log_str(format!("Sending ~{}Ⓝ to account @{}", amount_u128, account_id).as_str());
            
            let account_id:AccountId = account_id.parse().unwrap();
            let mut choicer = self.choicers
                .get(&account_id)
                .expect(&(format!("No choicer with id @{}",account_id)));

            choicer.completed_choices += 1;
            choicer.total_received += amount as Balance;
            choicer.current_choices -= 1;
            
            self.choicers.insert(&account_id,&choicer);

            Promise::new(account_id).transfer(amount_u128);
        }

        let mut choicer = self.choicers
                .get(&predecessor)
                .expect(&(format!("No choicer with id @{}",predecessor)));

            choicer.completed_choices += 1;
            choicer.current_choices -= 1;
            choicer.total_spending += total_spending;

            self.choicers.insert(&predecessor,&choicer);

    } 
}

//Converter helper
fn yton(yocto_amount: Balance) -> Balance {
    yocto_amount  / 10u128.pow(24)
}
fn ntoy(near_amount: Balance) -> Balance {
    near_amount * 10u128.pow(24)
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use super::*;
	use near_sdk::{ AccountId, MockedBlockchain };
    use near_sdk::{testing_env, VMContext, VMConfig, RuntimeFeesConfig};
    fn creator() -> String {
        "creator.near".to_string()
    }
    fn participant_1() -> String {
        "participant_1.near".to_string()
    }
    fn participant_2() -> String {
        "participant_2.near".to_string()
    }
    fn participant_3() -> String {
		"participant_3.near".to_string()
	}
	fn participant_4() -> String {
		"participant_4.near".to_string()
	}
	fn participant_5() -> String {
		"participant_5.near".to_string()
	}
	fn participant_6() -> String {
		"participant_6.near".to_string()
	}
	fn participant_7() -> String {
		"participant_7.near".to_string()
	}


	fn bob() -> String {
        "bob.near".to_string()
    }
	fn alice() -> String {
        "alice.near".to_string()
    }
    fn get_context(predecessor_account_id: String) -> VMContext {
        VMContext {
            current_account_id: alice(),
            signer_account_id: bob(),
            signer_account_pk: vec![0, 1, 2],
			attached_deposit: 1050_000_000_000_000_000_000_000_000, //1050Ⓝ 
            predecessor_account_id,
            input: vec![],
            block_index: 0,
            block_timestamp: 0,
            account_balance: 0,
            account_locked_balance: 0,
            storage_usage: 100000,
            prepaid_gas: 10u64.pow(18),
            random_seed: vec![0, 1, 2],
            is_view: false,
            output_data_receivers: vec![],
            epoch_height: 19,
        }
    }
	
    #[test]
    fn test_create_proposals() {
		//CREATOR CONTEXT. CREATE PROPOSALS
		testing_env!(
			get_context(creator())
		);
		env::log_str(format!("Balance ~{} Ⓝ for account @{}", yton(env::account_balance()), creator()).as_str());
        let mut contract = Contract::new();
		contract.create_membership();
        contract.create_proposal(
			VoteType::ProjectElection,
			"create logo".to_string(),
            10_000_000_000_000_000_000_000_000, // 10Ⓝ
			10,
			"we need logo for us".to_string()
		);
		contract.change_funds("creator.near001".to_string(), 200.0); // 10Ⓝ -> 200Ⓝ
		println!("{:?}", contract.view_active_proposals().to_vec());
		//PARTICIPANT_1 CONTEXT. CREATE MEMBERSHIP AND SUBMIT DECISION
		testing_env!(
			get_context(participant_1())
		);
        contract.create_membership();
		println!("{:?}", contract.is_active_proposal("creator.near001".to_string()));
		println!("{:?}", contract.is_a_member("participant_1.near".parse().unwrap()));
		contract.submit_decision("creator.near001".to_string(), "metadadalink1".to_string());
        //PARTICIPANT_2 CONTEXT. CREATE MEMBERSHIP AND SUBMIT DECISION
		testing_env!(
			get_context(participant_2())
		);
        contract.create_membership();
		println!("{:?}", contract.is_a_member("participant_2.near".parse().unwrap()));
		contract.submit_decision("creator.near001".to_string(), "metadadalink2".to_string());
		//PARTICIPANT_3 CONTEXT. CREATE MEMBERSHIP & SUBMIT DECISION
		testing_env!(
			get_context(participant_3())
		);
        contract.create_membership();
		println!("{:?}", contract.is_a_member("participant_3.near".parse().unwrap()));
		contract.submit_decision("creator.near001".to_string(), "metadadalink3".to_string());
        //PARTICIPANT_4 CONTEXT. CREATE MEMBERSHIP & SUBMIT DECISION
		testing_env!(
			get_context(participant_4())
		);
        contract.create_membership();
		println!("{:?}", contract.is_a_member("participant_4.near".parse().unwrap()));
		contract.submit_decision("creator.near001".to_string(), "metadadalink4".to_string());
         //PARTICIPANT_5 CONTEXT. CREATE MEMBERSHIP & SUBMIT DECISION
		testing_env!(
			get_context(participant_5())
		);
        contract.create_membership();
		println!("{:?}", contract.is_a_member("participant_5.near".parse().unwrap()));
		contract.submit_decision("creator.near001".to_string(), "metadadalink5".to_string());
        //PARTICIPANT_6 CONTEXT. CREATE MEMBERSHIP AND SUBMIT DECISION
		testing_env!(
			get_context(participant_6())
		);
        contract.create_membership();
		println!("{:?}", contract.is_active_proposal("creator.near001".to_string()));
		println!("{:?}", contract.is_a_member("participant_6.near".parse().unwrap()));
		contract.submit_decision("creator.near001".to_string(), "metadadalink6".to_string());
        //PARTICIPANT_7 CONTEXT. CREATE MEMBERSHIP AND SUBMIT DECISION
		testing_env!(
			get_context(participant_7())
		);
        contract.create_membership();
		println!("{:?}", contract.is_active_proposal("creator.near001".to_string()));
		println!("{:?}", contract.is_a_member("participant_7.near".parse().unwrap()));
		contract.submit_decision("creator.near001".to_string(), "metadadalink7".to_string());

		//CREATOR CONTEXT. START ELECTION
		testing_env!(
			get_context(creator())
		);
        contract.start_election("creator.near001".to_string());

        //PARTICIPANT_1 CONTEXT. VOTE
		testing_env!(
			get_context(participant_1())
		);
		contract.vote(
			"creator.near001".to_string(),
		    HashMap::from([
				("participant_2.near".to_string(), 1.0),
				("participant_3.near".to_string(), 2.0),
				("participant_7.near".to_string(), 3.0),
				("participant_4.near".to_string(), 4.0),
				("participant_6.near".to_string(), 5.0),
				("participant_5.near".to_string(), 6.0),
				])
			);
		//PARTICIPANT_2 CONTEXT. VOTE
		testing_env!(
			get_context(participant_2())
		);
		contract.vote(
			"creator.near001".to_string(),
		    HashMap::from([
				("participant_1.near".to_string(), 1.0),
				("participant_3.near".to_string(), 2.0),
				("participant_4.near".to_string(), 3.0),
				("participant_5.near".to_string(), 4.0),
				("participant_6.near".to_string(), 5.0),
				("participant_7.near".to_string(), 6.0),
				])
			);
		//PARTICIPANT_3 CONTEXT. VOTE
		testing_env!(
			get_context(participant_3())
		);
		contract.vote(
			"creator.near001".to_string(),
		    HashMap::from([
				("participant_2.near".to_string(), 1.0),
				("participant_7.near".to_string(), 2.0),
				("participant_1.near".to_string(), 3.0),
				("participant_4.near".to_string(), 4.0),
				("participant_6.near".to_string(), 5.0),
				("participant_5.near".to_string(), 6.0),
				])
			);
		//PARTICIPANT_4 CONTEXT. VOTE
		testing_env!(
			get_context(participant_4())
		);
		contract.vote(
			"creator.near001".to_string(),
		    HashMap::from([
				("participant_7.near".to_string(), 1.0),
				("participant_2.near".to_string(), 2.0),
				("participant_1.near".to_string(), 3.0),
				("participant_3.near".to_string(), 4.0),
				("participant_6.near".to_string(), 5.0),
				("participant_5.near".to_string(), 6.0),
				])
			);
		//PARTICIPANT_5 CONTEXT. VOTE
		testing_env!(
			get_context(participant_5())
		);
		contract.vote(
			"creator.near001".to_string(),
		    HashMap::from([
				("participant_1.near".to_string(), 1.0),
				("participant_6.near".to_string(), 2.0),
				("participant_7.near".to_string(), 3.0),
				("participant_3.near".to_string(), 4.0),
				("participant_4.near".to_string(), 5.0),
				("participant_2.near".to_string(), 6.0),
				])
			);
		//PARTICIPANT_6 CONTEXT. VOTE
		testing_env!(
			get_context(participant_6())
		);
		contract.vote(
			"creator.near001".to_string(),
		    HashMap::from([
				("participant_1.near".to_string(), 1.0),
				("participant_5.near".to_string(), 2.0),
				("participant_7.near".to_string(), 3.0),
				("participant_3.near".to_string(), 4.0),
				("participant_4.near".to_string(), 5.0),
				("participant_2.near".to_string(), 6.0),
				])
			);
		//PARTICIPANT_7 CONTEXT. VOTE
		testing_env!(
			get_context(participant_7())
		);
		contract.vote(
			"creator.near001".to_string(),
		    HashMap::from([
				("participant_1.near".to_string(), 1.0),
				("participant_6.near".to_string(), 2.0),
				("participant_5.near".to_string(), 3.0),
				("participant_3.near".to_string(), 4.0),
				("participant_4.near".to_string(), 5.0),
				("participant_2.near".to_string(), 6.0),
				])
			);
        //CREATOR CONTEXT. START PAYOUT
		testing_env!(
			get_context(creator())
		);
		println!("{:?}", contract.view_decisions("creator.near001".to_string()));
		println!("{:?}", contract.view_vote_board("creator.near001".to_string()));

		contract.finish_election("creator.near001".to_string());
		contract.payout("creator.near001".to_string());
        println!("{:?}", contract.choicers.get(&"participant_1.near".parse().unwrap()));
		println!("{:?}", contract.choicers.get(&"participant_2.near".parse().unwrap()));
		println!("{:?}", contract.choicers.get(&"participant_3.near".parse().unwrap()));
		println!("{:?}", contract.choicers.get(&"participant_4.near".parse().unwrap()));
		println!("{:?}", contract.choicers.get(&"participant_5.near".parse().unwrap()));
		println!("{:?}", contract.choicers.get(&"participant_6.near".parse().unwrap()));
		println!("{:?}", contract.choicers.get(&"participant_7.near".parse().unwrap()));
		println!("{:?}", contract.choicers.get(&"creator.near".parse().unwrap()));

    }

}
