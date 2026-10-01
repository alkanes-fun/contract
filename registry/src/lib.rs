








































use alkanes_runtime::runtime::AlkaneResponder;
use alkanes_runtime::storage::StoragePointer;
use alkanes_runtime::{declare_alkane, message::MessageDispatch};
use alkanes_support::{
    id::AlkaneId,
    parcel::{AlkaneTransfer, AlkaneTransferParcel},
    response::CallResponse,
};
use anyhow::{anyhow, Result};
use alkanes_std_maga_common::abi;
use alkanes_std_maga_common::codec::{pack_u128s, unpack_u128s};
use alkanes_std_maga_common::math::{validate_curve_config, validate_max_buy, BPS};
use alkanes_std_maga_common::runtime::{id_words, pack_id, unpack_id, BytesPointer};
#[allow(unused_imports)]
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};
use metashrew_support::index_pointer::KeyValuePointer;


const PLATFORM_CREDENTIAL_NAME: &str = "MAGA platform";
const PLATFORM_CREDENTIAL_SYMBOL: &str = "MAGA-P";

#[derive(Default)]
pub struct MagaRegistry(());

#[derive(MessageDispatch)]
enum MagaRegistryMessage {


    #[opcode(0)]
    Initialize {
        platform_auth: AlkaneId,
        launch_template: AlkaneId,
        creator_bps: u128,
        supply: u128,
        target: u128,
        granularity: u128,
        deadline_offset: u128,
        pool_template: AlkaneId,
        swap_fee_bps: u128,
        credential_template: AlkaneId,
        quote: AlkaneId,
    },




    #[opcode(10)]
    CreateProject {
        name: String,
        symbol: String,
        creator_auth: AlkaneId,
        open_height: u128,
        deadline: u128,
        max_buy: u128,
    },

    #[opcode(11)]
    #[returns(Vec<u8>)]
    GetProject { project_id: u128 },

    #[opcode(16)]
    #[returns(u128)]
    GetProjectCount,

    #[opcode(90)]
    #[returns(Vec<u8>)]
    GetConfig,

    #[opcode(99)]
    #[returns(String)]
    GetName,
}


fn p_platform_auth() -> StoragePointer {
    StoragePointer::from_keyword("/platform_auth")
}
fn p_launch_template() -> StoragePointer {
    StoragePointer::from_keyword("/launch_template")
}
fn p_creator_bps() -> StoragePointer {
    StoragePointer::from_keyword("/creator_bps")
}
fn p_supply() -> StoragePointer {
    StoragePointer::from_keyword("/supply")
}
fn p_target() -> StoragePointer {
    StoragePointer::from_keyword("/target")
}
fn p_granularity() -> StoragePointer {
    StoragePointer::from_keyword("/granularity")
}
fn p_deadline_offset() -> StoragePointer {
    StoragePointer::from_keyword("/deadline_offset")
}
fn p_pool_template() -> StoragePointer {
    StoragePointer::from_keyword("/pool_template")
}
fn p_swap_fee_bps() -> StoragePointer {
    StoragePointer::from_keyword("/swap_fee_bps")
}
fn p_credential_template() -> StoragePointer {
    StoragePointer::from_keyword("/credential_template")
}


fn p_quote() -> StoragePointer {
    StoragePointer::from_keyword("/quote")
}

fn p_count() -> StoragePointer {
    StoragePointer::from_keyword("/count")
}
fn p_project(project_id: u128) -> StoragePointer {
    StoragePointer::from_keyword("/proj").select(&project_id.to_le_bytes().to_vec())
}

impl MagaRegistry {
    #[allow(clippy::too_many_arguments)]
    fn initialize(
        &self,
        platform_auth: AlkaneId,
        launch_template: AlkaneId,
        creator_bps: u128,
        supply: u128,
        target: u128,
        granularity: u128,
        deadline_offset: u128,
        pool_template: AlkaneId,
        swap_fee_bps: u128,
        credential_template: AlkaneId,
        quote: AlkaneId,
    ) -> Result<CallResponse> {
        self.observe_initialization()?;
        if [&launch_template, &pool_template, &credential_template]
            .iter()
            .any(|template| template.block != 2)
        {
            return Err(anyhow!("templates must be deployed contracts (block 2)"));
        }
        if creator_bps > BPS {
            return Err(anyhow!("creator bps out of range"));
        }

        if swap_fee_bps >= BPS {
            return Err(anyhow!("swap fee bps out of range"));
        }
        validate_curve_config(supply, target, granularity)?;

        p_credential_template().set_bytes(pack_id(&credential_template));
        let minted = platform_auth == AlkaneId::default();
        let platform_auth = if minted {
            self.mint_credential(PLATFORM_CREDENTIAL_NAME, PLATFORM_CREDENTIAL_SYMBOL)?
        } else {
            platform_auth
        };
        p_platform_auth().set_bytes(pack_id(&platform_auth));
        p_launch_template().set_bytes(pack_id(&launch_template));
        p_creator_bps().set_value::<u128>(creator_bps);
        p_supply().set_value::<u128>(supply);
        p_target().set_value::<u128>(target);
        p_granularity().set_value::<u128>(granularity);
        p_deadline_offset().set_value::<u128>(deadline_offset);
        p_pool_template().set_bytes(pack_id(&pool_template));
        p_swap_fee_bps().set_value::<u128>(swap_fee_bps);
        p_quote().set_bytes(pack_id(&quote));
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);
        if minted {
            response.alkanes.0.push(AlkaneTransfer {
                id: platform_auth,
                value: 1,
            });
        }
        Ok(response)
    }





    fn mint_credential(&self, name: &str, symbol: &str) -> Result<AlkaneId> {
        let credential = AlkaneId {
            block: 2,
            tx: self.sequence(),
        };
        let template = unpack_id(&p_credential_template().get_bytes())?;
        let created = self.call(
            &abi::credential::clone_and_initialize(&template, name, symbol, 1),
            &AlkaneTransferParcel::default(),
            self.fuel(),
        )?;
        if !created.alkanes.0.iter().any(|t| t.id == credential && t.value == 1) {
            return Err(anyhow!("credential template did not mint one unit of the predicted id"));
        }
        Ok(credential)
    }



    fn create_project(
        &self,
        name: String,
        symbol: String,
        creator_auth: AlkaneId,
        open_height: u128,
        deadline: u128,
        max_buy: u128,
    ) -> Result<CallResponse> {
        let context = self.context()?;


        let launch_template = unpack_id(&p_launch_template().get_bytes())?;
        let quote = unpack_id(&p_quote().get_bytes())?;
        let platform_auth = unpack_id(&p_platform_auth().get_bytes())?;
        let project_id = p_count().get_value::<u128>() + 1;



        let open_height = if open_height == 0 {
            self.height() as u128
        } else {
            open_height
        };
        let deadline = if deadline == 0 {
            open_height
                .checked_add(p_deadline_offset().get_value::<u128>())
                .ok_or_else(|| anyhow!("deadline overflow"))?
        } else {
            deadline
        };



        if deadline < open_height {
            return Err(anyhow!("deadline before open height"));
        }
        validate_max_buy(p_granularity().get_value::<u128>(), max_buy)?;




        let minted = creator_auth == AlkaneId::default();
        let creator_auth = if minted {
            self.mint_credential(&format!("{name} creator"), &format!("{symbol}-C"))?
        } else {
            creator_auth
        };




        let launch = AlkaneId {
            block: 2,
            tx: self.sequence(),
        };
        let pool = AlkaneId {
            block: 2,
            tx: launch.tx + 1,
        };


        for auth in [&creator_auth, &platform_auth] {
            if auth == &quote || auth == &launch {
                return Err(anyhow!("credential asset cannot be the quote asset or the project token"));
            }
        }
        if [&quote, &creator_auth, &platform_auth].contains(&&pool) {
            return Err(anyhow!("quote and credential assets cannot be the project's pool"));
        }






        self.call(
            &abi::launch::clone_and_initialize(
                &launch_template,
                &abi::launch::Init {
                    project_id,
                    quote,
                    pool,
                    creator_auth,
                    platform_auth,
                    creator_bps: p_creator_bps().get_value::<u128>(),
                    open_height,
                    deadline,
                    supply: p_supply().get_value::<u128>(),
                    target: p_target().get_value::<u128>(),
                    granularity: p_granularity().get_value::<u128>(),
                    name: &name,
                    symbol: &symbol,
                    max_buy,
                },
            ),
            &AlkaneTransferParcel::default(),
            self.fuel(),
        )?;






        let created = self.call(
            &abi::pool::clone_and_initialize(
                &unpack_id(&p_pool_template().get_bytes())?,
                &abi::pool::Init {
                    project: project_id,
                    launch,
                    quote,
                    registry: context.myself,
                    platform_auth,
                    swap_fee_bps: p_swap_fee_bps().get_value::<u128>(),
                },
            ),
            &AlkaneTransferParcel::default(),
            self.fuel(),
        )?;


        if abi::pool::decode_init_receipt(&created.data)? != pool {
            return Err(anyhow!("pool template reported an unexpected pool id"));
        }




        let mut record: Vec<u128> = vec![project_id];
        record.extend(id_words(&launch));
        record.extend(id_words(&quote));
        record.extend(id_words(&creator_auth));
        record.push(open_height);
        record.push(deadline);
        record.extend(id_words(&pool));
        p_project(project_id).set_bytes(pack_u128s(&record));
        p_count().set_value::<u128>(project_id);

        let mut response = CallResponse::forward(&context.incoming_alkanes);
        if minted {
            response.alkanes.0.push(AlkaneTransfer {
                id: creator_auth,
                value: 1,
            });
        }
        response.data = pack_u128s(&[
            project_id,
            launch.block,
            launch.tx,
            pool.block,
            pool.tx,
            creator_auth.block,
            creator_auth.tx,
        ]);
        Ok(response)
    }

    fn get_project(&self, project_id: u128) -> Result<CallResponse> {
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);
        response.data = p_project(project_id).get_bytes();
        Ok(response)
    }

    fn get_project_count(&self) -> Result<CallResponse> {
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);
        response.data = p_count().get_value::<u128>().to_le_bytes().to_vec();
        Ok(response)
    }




    fn get_config(&self) -> Result<CallResponse> {
        let mut data = vec![
            p_creator_bps().get_value::<u128>(),
            p_supply().get_value::<u128>(),
            p_target().get_value::<u128>(),
            p_granularity().get_value::<u128>(),
            p_deadline_offset().get_value::<u128>(),
        ];
        data.extend(unpack_u128s(&p_launch_template().get_bytes())?);
        data.extend(unpack_u128s(&p_platform_auth().get_bytes())?);
        data.extend(unpack_u128s(&p_pool_template().get_bytes())?);
        data.push(p_swap_fee_bps().get_value::<u128>());
        data.extend(unpack_u128s(&p_credential_template().get_bytes())?);
        data.extend(unpack_u128s(&p_quote().get_bytes())?);
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);
        response.data = pack_u128s(&data);
        Ok(response)
    }

    fn get_name(&self) -> Result<CallResponse> {
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);
        response.data = b"MAGA Registry".to_vec();
        Ok(response)
    }
}

impl AlkaneResponder for MagaRegistry {}

declare_alkane! {
    impl AlkaneResponder for MagaRegistry {
        type Message = MagaRegistryMessage;
    }
}
