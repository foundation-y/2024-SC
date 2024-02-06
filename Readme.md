# Requirements

The project was build with: `cargo 1.75.0`, `rustc 1.75.0`, `node v19.9.0`, `yarn 1.22.21`,
`wasm-opt 105`.

# Build

Run `cargo build --release --lib --target wasm32-unknown-unknown` to build smart contract. Wasm files will be located in the
`./artifacts` directory.

# Tier

The smart contract accepts delegations from users to define their `Tier`. `Tier`
value defines the amount of tokens the buyer can buy in the IDO contract.

## Deploy

Check oraid version:

```bash
oraid version
# Should return 0.41.0 or higher
```

Run:

```bash
WALLET="my wallet name"
WALLET_ADDRESS="my wallet address"
TIER_LABEL="tier contract"

# Choose a validator
oraid query staking validators

# For example, we choose this one
VALIDATOR="oraivaloper1f9judw4xg7d8k4d4ywgz8wsxvuesur739sr88g"

oraid config broadcast-mode block

# Find code_id value from the output
oraid tx wasm store artifacts/tier.wasm --from $WALLET --gas auto --gas-adjustment 1.3 -y

# example code id
TIER_CODE_ID="6673"

# testnet
USDT_CONTRACT="orai1laj3d4zledty0r0vd7m3gem4cd7cyk09m608863p3t7p6sm6xmusru4l5p",  
ORAI_SWAP_ROUTER_CONTRACT="orai1e4k9zhjpz3a0q6fgspwj3ug5fhc3t2emtxuvtra79hs70gqq7m0sg8kdd5"

# mainnet
# BAND_CONTRACT=secret1ezamax2vrhjpy92fnujlpwfj2dpredaafss47k

# instantiate. Here, validators are the array of validator addresses and weights. Total sum of weights have to be 100.
oraid tx wasm instantiate "$TIER_CODE_ID" \
 '{
"validators": [{
"address": "oraivaloper18hr8jggl3xnrutfujy2jwpeu0l76azprkxn29v",
"weight": "100"
}],  
 "oraiswap_contract": {  
 "usdt_contract": "'"${USDT_CONTRACT}"'",  
 "orai_swap_router_contract": "'"${ORAI_SWAP_ROUTER_CONTRACT}"'"
},
"deposits": ["25000", "7500", "1500", "250"],
"admin":"'"${WALLET_ADDRESS}"'"
}'                      \
 --gas auto             \
 --gas-adjustment 1.1   \
 --gas-prices 0.1orai   \
 --no-admin             \
 --from $WALLET         \
 --label "YOUI_ORAI"    \
 --yes
```

Check the initialization with:

```bash
# It will print the smart contract address
oraid query wasm list-contract-by-code "$TIER_CODE_ID"

TIER_ADDRESS=$(oraid query wasm list-contract-by-code "$TIER_CODE_ID" --output json |
    jq -r '.contracts[0]')
```

## Usage

To deposit some ORAI, run:

```bash
oraid tx wasm execute "$TIER_ADDRESS" \
    '{ "deposit": {} }'                      \
    --from "$WALLET"                         \
    --amount 30000000orai                       \
    --yes
```

To check your tier:

```bash
oraid q wasm contract-state smart "$TIER_ADDRESS" \
    '{ "user_info": {"address":"'"$WALLET_ADDRESS"'"} }'

# {"data":{"user_info":{"tier":5,"timestamp":1671696042,"usd_deposit":"150","orai_deposit":"24.9"}}}
```

To withdraw your ORAI:

```bash
oraid tx wasm execute "$TIER_ADDRESS" \
    '{ "withdraw": {} }'                     \
    --from "$WALLET"                         \
    --yes
```

Claim your money after unbound period:

```bash
oraid tx wasm execute "$TIER_ADDRESS" \
    '{ "claim": {} }'                        \
    --from "$WALLET"                         \
    --yes
```

# IDO

The smart contract for the IDO platform.

## Deploy

Run:

```bash
oraid tx wasm store artifacts/ido.wasm \
    --gas 2700000                           \
    --from "$WALLET"                        \
    --yes

# example code id
IDO_CODE_ID="6674"
```

Instantiate contract:

```bash
NFT_ADDRESS="nft contract address"

oraid tx wasm instantiate                             \
    "$IDO_CODE_ID"                                           \
    '{
        "lock_periods": [864000, 1209600, 1209600, 1209600, 1209600],
        "nft_contract": "'"${NFT_ADDRESS}"'",
        "tier_contract": "'"${TIER_ADDRESS}"'",
    }'                                                       \
    --gas auto                                               \
    --gas-adjustment 1.1                                     \
    --gas-prices 0.1orai                                     \
    --from "$WALLET"                                         \
    --label "$IDO_LABEL"                                     \
    --yes
```

Check the initialization with:

```bash
# It will print the smart contract address
oraid query wasm list-contract-by-code "$IDO_CODE_ID"

IDO_ADDRESS=$(oraid query wasm list-contract-by-code "$IDO_CODE_ID" --output json |
    jq -r '.contracts[0]')
```

## Usage

Create IDO:

```bash
AMOUNT=1000000000000
TOKENS_PER_TIER='["400000000000", "300000000000", "150000000000", "100000000000", "50000000000"]'

CW20_CONTRACT_ADDRESS="cw20 token contract address"

oraid tx wasm execute "$CW20_CONTRACT_ADDRESS"    \
    '{
        "increase_allowance": {
            "spender": "'"$IDO_ADDRESS"'",
            "amount": "'"$AMOUNT"'"
        }
    }'                                             \
    --from "$WALLET"                               \
    --yes

# shared whitelist
WHITELIST_OPTION='{"shared": {}}'

# empty whitelist
# WHITELIST_OPTION='{"empty": {}}'

START_TIME=$(date -d "now - 5 minutes" +%s)
END_TIME=$(date -d "now + 5 minutes" +%s)
PRICE=50
SOFT_CAP=5000
PAYMENT_TOKEN_OPTION="native"

oraid tx wasm execute "$IDO_ADDRESS"                    \
    '{
        "start_ido": {
            "start_time": '"${START_TIME}"',
            "end_time": '"${END_TIME}"',
            "total_amount": "'"$AMOUNT"'",
            "tokens_per_tier": '"${TOKENS_PER_TIER}"',
            "price": "'"${PRICE}"'",
            "token_contract": "'"${IDO_TOKEN}"'",
            "payment": '"${PAYMENT_TOKEN_OPTION}"',
            "whitelist": '"${WHITELIST_OPTION}"',
            "soft_cap": "'"$SOFT_CAP"'"
        }
    }'                                                         \
    --from "$WALLET"                                           \
    --yes
```

Add whitelist:

```bash
IDO_ID=0

oraid tx wasm execute "$IDO_ADDRESS" \
    '{
        "whitelist_add": {
            "addresses": ["user address"],
            "ido_id": '"${IDO_ID}"'
        }
    }'                                      \
    --from "$WALLET"                        \
    --yes
```

Buy some tokens:

```bash
IDO_ID=0
AMOUNT=2000000000

# amount * price
MONEY=40000000

oraid tx wasm execute "$CW20_CONTRACT_ADDRESS" \
    '{
        "increase_allowance": {
            "spender": "'"$IDO_ADDRESS"'",
            "amount": "'"$MONEY"'"
        }
    }'                                        \
    --from "$WALLET"                          \
    --yes

oraid tx wasm execute "$IDO_ADDRESS" \
    '{
        "buy_tokens": {
            "amount": "'"$AMOUNT"'",
            "ido_id": '"$IDO_ID"'
        }
    }'                                      \
    --from "$WALLET"                        \
    --gas 500000                           \
    --yes
```

Receive tokens after lock period:

```bash
oraid tx wasm execute "$IDO_ADDRESS" \
    '{
        "recv_tokens": {
            "ido_id": '"$IDO_ID"'
        }
    }'                                      \
    --from "$WALLET"                        \
    --gas 500000                           \
    --yes
```
