# Configuration Handling with Clap & Figment

Uses **Clap** (for CLI parsing) together with **Figment** (for layered configuration) to provide a flexible way to config.

Target:
- Every option can be set **via CLI flags**, **environment variables**, **config file**, or fall back to a **default**.  
- Config resolution order:  
  **CLI > Config file > Environment > Defaults**  

---

## 1. CLI Options

Define CLI options using `#[derive(Parser)]` from **Clap**:
- Each option should be in `Option<T>` by default.  
- No hardcoded defaults in `#[arg(default_value = ...)]`. Instead, defaults live in the `Config` struct.
- No env var set in `[arg(env = ...)]`. Each option automatically gets an **environment variable override** by using the pattern: `PREFIX + UPPERCASE(var)`


Example:

```rust
#[derive(Parser, Debug, Serialize, Deserialize)]
pub struct Cli {
    #[arg(long, help = "...")] // no env or default here
    pub port: Option<u16>,
}

// with the Env::prefixed set like below, the env variable correspond with port is PREFIX + UPPERCASE(port) = ETHREX_PORT
pub(crate) fn load_config(cli: Cli) -> Result<Config, Box<figment::Error>> {
    let figment = Figment::new()
        .merge(Serialized::defaults(Config::default()))
        .merge(Env::prefixed("ETHREX_"))
        .merge(Json::file("mojave/node.setting.json"))
        .merge(Serialized::<Cli>::defaults(cli))
        .extract()?;

    Ok(figment)
}

```

## 2. Config Struct and Flattened CLI Structures

Ofcourse, we need to `Config` struct which contain every possible fields needed to config:
- This `Config` struct as mentioned above, should implement `Default`
- To support serialize and layered config:
    - `Config` should name every fields after options in `Cli` struct
    - `Config` and `Cli` must have same serialize. `Cli` struct must use `serde[(flatten)]` for nested options, and `serde[(untagged)]` to ignore meaningless command
    - `Cli`, alongside with `Option<T>` type, is recommended to have `#[serde(skip_serializing_if = "::std::option::Option::is_none")]`, this allow config to merge by order while ignore `None` value

## 3. Config Resolution (Figment)

We merge sources in the following order:
1. CLI arguments (highest precedence)
2. Config file (`.toml`, `.yaml`, `.json`), though `Figment` support all 3 types, for now, we only use `.json`
3. Environment variables (set with prefixed)
4. Defaults (from Config::default)

Example implementation: (found in [config.rs](../cmd/node/src/config.rs))
```rust
pub(crate) fn load_config(cli: Cli) -> Result<Config, Box<figment::Error>> {
    let figment = Figment::new()
        .merge(Serialized::defaults(Config::default()))
        .merge(Env::prefixed("ETHREX_"))
        .merge(Json::file("mojave/node.setting.json"))
        .merge(Serialized::<Cli>::defaults(cli))
        .extract()?;

    Ok(figment)
}
```


## 4. Config file format

This follow example contain most of use case for `.json` file, from `numeric`, `string` to `enum` options

```json
{
  "log_level": "debug",
  "network": { "GenesisPath": "/path/to/genesis.json" },
  "p2p_enabled": true,
  "http_port": "8545"
}
```