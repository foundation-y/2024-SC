use crate::msg::ExecuteMsg;
use cosmwasm_std::{ to_json_binary, Addr, CosmosMsg, StdResult, WasmMsg };
use schemars::JsonSchema;
use serde::{ Deserialize, Serialize };

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct CwTemplateContract(pub Addr);

impl CwTemplateContract {
    pub fn addr(&self) -> Addr {
        self.0.clone()
    }

    pub fn call<T: Into<ExecuteMsg>>(&self, msg: T) -> StdResult<CosmosMsg> {
        let msg = to_json_binary(&msg.into())?;
        Ok(
            (WasmMsg::Execute {
                contract_addr: self.addr().into(),
                msg,
                funds: vec![],
            }).into()
        )
    }
}
