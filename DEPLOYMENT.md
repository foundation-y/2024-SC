# Deployment Information

Create a `.env` file in the root directory. Follow content in `.env.example` to use as template

## Environment File

-   MNEMONIC = MNEMONIC of deploying account
-   NETWORK = either `testnet` or `mainnet`

### Deploying to Injective

```sh
bun ./scripts/deployer.inj.ts
```

### Deploying to OraiChain

```sh
bun ./scripts/deployer.orai.ts
```
