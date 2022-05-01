# CHOICE 
![img](https://github.com/YellingOilbird/choice/assets/logo2.svg)] 
## Smart contract on NEAR blockchain

It is a blockchain-based app which allows you to place a proposal and receive decisions from other registered users.
Usually the winner (the performer of the best decision) takes all the reward, which is not really fair, right?
So here we have implemented a functionality that provides a closed non-binary voting between proposal participants for the best decision.

### App logic
### Proposal created by Choice member ```creator.near```
```rust
(
			VoteType::ProjectElection,          
			"create logo".to_string(),                    // title  
                        100_000_000_000_000_000_000_000_000,          // 100Ⓝ  
			3,                                            // number of max_decisions  
			"we need logo for our project".to_string()    // sample description  
		);
```
##### Choice members submit their decisions while proposal is open for it
👨```participant_1.near``` =>  http://link_to_my_logo_for_you/from_participant_1.near   
👨```participant_2.near``` =>  http://link_to_my_logo_for_you/from_participant_2.near  
👨```participant_3.near``` =>  http://link_to_my_logo_for_you/from_participant_3.near  
~~👤```participant_4.near``` =>  http://link_to_my_logo_for_you/from_participant_4.near~~  
*number of max_decisions = 3, sorry, ```participant_4.near```, you are late*
#####
---
### So, time is over. Now we don't take any decisions. Vote stage is starting here
Every user, who submit decision into this proposal is eligible to vote. Of course, you cannot vote for self.
It will be fully realized on frontend side. Here is this process:
**format - account : place**  

👨```participant_1.near``` =>  {"participant_2.near": 1.0,..."participant_3.near": 2.0}  
👨```participant_2.near``` =>  {"participant_1.near": 1.0,..."participant_3.near": 2.0}  
👨```participant_3.near``` =>  {"participant_1.near": 1.0,..."participant_2.near": 2.0}  

What we see right here: 
- participant_1.near appears two times at 1st place,
- participant_2.near appears one time at 1st place, and one time at 2nd place,
- participant_3.near appears two times at 3rd place
### Vote stage finished. Vote engine calculate all votes like this:
w1 - weight multiply for 1st place = 2 * w2 
w2 - weight multiply for 2nd place 
```rust
participant_1.near = 2 * w1  = 2 * 2 * w2  = 4 * w2
participant_2.near = w1 + w2 = 2 * w2 + w2 = 3 * w2 
participant_3.near = 2 * w2                = 2 * w2 
```
*So, we have 9 similar w2 weights. Vote engine takes attached to proposal funds and calculate minimal weight. For our example it is 100Ⓝ*

```rust
w2 = 100 / 9 = 11.1
//calculate for all participants...
participant_1.near = 4 * w2 = 44.4
participant_2.near = 3 * w2 = 33.3
participant_3.near = 2 * w2 = 22.2
```
###### Finally we are disperse funds to all participants.
```rust
Sending ~44Ⓝ to account @participant_1.near
Sending ~33Ⓝ to account @participant_2.near
Sending ~22Ⓝ to account @participant_3.near
```
### Done! After this contract refresh info about each participant and creator, counting completed/current choices and received/spending money for proposals to collect users data. This data will be used in the future for loyalty airdrops for voters and creators

#### Usage:

```bash
$cargo test -- --nocapture`
$RUSTFLAGS='-C link-arg=-s' cargo build --target wasm32-unknown-unknown --release`
```
```bash
$near create-account <ACCOUNT.MASTERACCOUNT> --masterAccount <MASTERACCOUNT>
$near deploy <ACCOUNT.MASTERACCOUNT> --wasmFile res/choice.wasm'
```

---

#### Methods:
*```near```commands wiil be right here little bit later*  
*⚰️ methods wiil deprecated and changed to automatic based on Duration functions*

```create_membership()```                    - create new membership in app  
##### CREATOR SIDE
```create_proposal(...)```                   - create new proposal   
```change_funds(proposal_id, new funds)```   - change proposal attached funds (only before voting starts!)   
```view_decisions(proposal_id)```            - returns all submitted decisions for proposal    
*⚰️* ```start_election(proposal_id)```       - starts Vote phase  
*⚰️* ```finish_election(proposal_id)```      - finish Vote phase   
##### CHOICER SIDE 
```view_active_proposals()```                 - returns all open proposals    
```submit_decision(proposal_id, metadata)```  - submit your decision in proposal (it can be link on github)     
```vote(proposal_id, vote)```                 - vote in format ```{"account_1.near": 1.0,..."account_n.near": n.0  
```view_vote_board(proposal_id)```            - returns all votes for proposal  
##### VOTE ENGINE       
*⚰️*```payout(proposal_id)```                 - disperse funds according to the vote results  

Enjoy!  



## License
[MIT](https://choosealicense.com/licenses/mit/)
