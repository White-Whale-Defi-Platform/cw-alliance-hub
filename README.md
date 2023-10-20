# CW Alliance Hub Migaloo
### Intent

Stake idle CW20s for redirected inflation through Alliance
Stake unbonded LP tokens for redirected inflation through Alliance

For Chain
Stimulate the economies of projects by redirecting some inflation to holders who stake their token based on configurable weighting.

#### Contract methods 

##### Instantiate 
When instantiating a number of actor addrs must be provided for the governance, the controller and the oracle. These can all be set to the same address if desired but its split to allow better role based access

```mermaid
graph TD
A(Asset Whitelisted) --> B(Holders of Asset can Stake)
B --> C(Holders stake to contract)
C --> D(Rewards Updated)
D --> E(Stakers claim rewards for staked asset)
```






##### Whitelisting Assets 

Assets are whitelisted before users can stake and unstake. 
Additionally the reward distribution must be set for any whitelisted asset to receive some redirected inflation assets. 

Whitelisting an asset is done by an admin/governance role and can be done like so: 

```bash
./migalood tx wasm execute migaloo14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9s58v48z '{ "whitelist_assets": {"test-chain-GKFJpU": [{"cw20": "migaloo1xr3rq8yvd7qplsw5yx90ftsr2zdhg4e9z60h5duusgxpv72hud3s54xttx"}]}' --from new_deploy_wallet --gas auto --gas-adjustment 1.4
```
After which any holders of that asset can now stake and unstake. Rewards will only be accumulated if the asset is also included in the reward_distribution.

##### Reward Distribution 
The Reward Distribution represents a vector of assets and their respective weights for the sharing of the redirected inflation 
The weights must add to 100% when being set. 

Important to note reward distribution is separate from whitelisting for a reason. This enables assets to be whitelisted before any rewards are shared and additionally ensures rewards are independent of the whitelist.

Also important to note setting new reward distributions is a one-hot operation that needs to be done with the entire list of distribution. 
This removes any recalculation logic from the contract and enforces that the governance/admin actor must provide any updated rates and that despite updates they all add to 100%. 

Example: 
3 assets are whitelisted and a reward distribution is set at 33% for each. 
```mermaid

pie title ASSET_REWARD_DISTRIBUTION 
		"Fable" : 33
		"Racoon" : 33
		"Ginkou" : 33
```




At some point in the future this reward distribution can be re-weighted 30,20,40 without touching the whitelist 
```mermaid

pie title ASSET_REWARD_DISTRIBUTION 
		"Fable" : 30
		"Racoon" : 30
		"Ginkou" : 40
```


A fourth asset is whitelist and the reward distribution is set again: 

```bash
./migalood tx wasm execute migaloo14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9s58v48z '{"set_asset_reward_distribution": [{"asset": {"native": "factory/addr/fable"}, "distribution": "0.25"}, {"asset":{"cw20":"migaloo1xr3rq8yvd7qplsw5yx90ftsr2zdhg4e9z60h5duusgxpv72hud3s54xttx"}, "distribution": "0.3"}, {"asset": {"cw20":"migaloo1anotherone"}, "distribution": "0.4"}, {"asset": {"native": "factory/migaloo1v767q4apajgksqlg5ejdakn8auszecje3yqfw6/fable"}, "distribution": "0.05"}]}'
```

The rewards for all assets have been updated at once
```mermaid

pie title ASSET_REWARD_DISTRIBUTION 
		"Fable" : 25
		"Racoon" : 30
		"Ginkou" : 40
		"anotherone": 5
```


##### Updating rewards
Updating rewards is the process in which all earned rewards from the redirect inflation is tallied by querying the balance of the reward denom on the contract. 

For each validator in the set VALIDATORS state item, an Alliance `MsgClaimDelegationRewards` is prepared to be sent as well as a UpdateRewardsCallback for the contract where newly received rewards will be allocated. 

In the Callback, the amount of newly gained assets is tallied to determine the rewards_collected. 
These rewards_collected are then allocated based on the assets distribution percent.
If there are not balances for a given asset, no rate update happens which also means no emissions are redirected to them. 

The above actions set an `ASSET_REWARD_RATE` for each asset which is then used when staking, unstaking or claiming via the `_claim_rewards`. All unclaimed rewards are gathered on stake, unstake and then claimed whenever claim_rewards is called.

#### User facing methods

##### Claiming rewards 
Rewards claimable by a staker of a whitelisted asset can claim their earned rewards after the needed steps in [[Alliance Hub Migaloo#Updating Rewards]] have been performed. This should be done on some interval meaning rewards are being accumulated steadily. 

Users in this scenario can claim their rewards one asset per time. 

The asset that must be passed is an alias of `AssetInfoBase<Addr>` from cw-asset. It looks like so 

```rust
pub type AssetInfo = AssetInfoBase<Addr>;

#[cw_serde]
#[non_exhaustive]
pub enum AssetInfoBase<T> {
	Native(String),
	Cw20(T),
	Cw1155(T, String),
}
```
The claiming message for a given asset may look like: 

```bash
./migalood tx wasm execute migaloo14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9s58v48z '{"claim_rewards": {"cw20": "migaloo1xr3rq8yvd7qplsw5yx90ftsr2zdhg4e9z60h5duusgxpv72hud3s54xttx"}}' --from new_deploy_wallet --gas auto --gas-adjustment 1.4

```
In the case of a CW20 token or 
```bash
./migalood tx wasm execute migaloo14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9s58v48z '{"claim_rewards": {"native": "factory/migaloo1v767q4apajgksqlg5ejdakn8auszecje3yqfw6/fable"}}' --from new_deploy_wallet --gas auto --gas-adjustment 1.4
```

In the case of a native token

Provided the needed steps for [[Alliance Hub Migaloo#Updating Rewards]] have been performed. All due rewards will be claimed for the requested asset.
This operation must be repeated for each different asset you may have staked. 



# Development

Considering the Rust is installed in your system you have to use the wasm32 compiler and install cargo-make. 

```sh
$ rustup default stable
$ rustup target add wasm32-unknown-unknown
$ cargo install --force cargo-make
```

There are few available commands to run on development:

Validate the code has been formatted correctly:
```sh
$ cargo make fmt
```

Run the tests written for the smart contracts
```sh
$ cargo make test
```

Lint the code 
```sh
$ cargo make lint
```

Generate json Schemas for each smart contract
```sh
$ cargo make schema
```

Build the code
```sh
$ cargo make build
```

Optimize the built code
```sh
$ cargo make optimize
```

# Deployment 

Before executing the following scripts, navigate to the scripts folder and execute `yarn` command to install the dependencies for the set of scripts. Also remember to set the following environment variables:

```sh
# Mnemonic of the account to deploy the contract with
MNEMONIC=
# Chain id where to deploy the contract
CHAIN_ID=
# Prefix of the acccounts where to deploy the smart contract 
ACC_PREFIX=
```

To deploy oracle and alliance hub smart contract:
```sh
$ cargo make deploy-oracle
$ cargo make deploy-hub
```
