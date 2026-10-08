





















































use alkanes_runtime::runtime::AlkaneResponder;
use alkanes_runtime::storage::StoragePointer;
use alkanes_runtime::{declare_alkane, message::MessageDispatch};
use alkanes_support::{
    cellpack::Cellpack,
    id::AlkaneId,
    parcel::{AlkaneTransfer, AlkaneTransferParcel},
    response::CallResponse,
};
use alkanes_std_maga_common::codec::{pack_u128s, unpack_u128s};
use alkanes_std_maga_common::runtime::{id_words, pack_id, unpack_id, BytesPointer};
use anyhow::{anyhow, Result};
#[allow(unused_imports)]
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};
use metashrew_support::index_pointer::KeyValuePointer;




pub fn get_project(registry: &AlkaneId, project_id: u128) -> Cellpack {
    Cellpack { target: *registry, inputs: vec![11, project_id] }
}

pub fn claim_platform_fee(launch: &AlkaneId) -> Cellpack {
    Cellpack { target: *launch, inputs: vec![3] }
}


pub fn collect_fees(pool: &AlkaneId) -> Cellpack {
    Cellpack { target: *pool, inputs: vec![10, 1] }
}




pub fn decode_project(data: &[u8], project_id: u128) -> Result<(AlkaneId, AlkaneId)> {
    match unpack_u128s(data)?[..] {
        [id, launch_block, launch_tx, _, _, _, _, _, _, pool_block, pool_tx] if id == project_id => Ok((
            AlkaneId { block: launch_block, tx: launch_tx },
            AlkaneId { block: pool_block, tx: pool_tx },
        )),
        [] => Err(anyhow!("the registry has no such project")),
        _ => Err(anyhow!("unexpected project record")),
    }
}


pub fn get_config(registry: &AlkaneId) -> Cellpack {
    Cellpack { target: *registry, inputs: vec![90] }
}




#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryConfig {
    pub creator_bps: u128,
    pub supply: u128,
    pub target: u128,
    pub granularity: u128,
    pub deadline_offset: u128,
    pub launch_template: AlkaneId,
    pub platform_auth: AlkaneId,
    pub pool_template: AlkaneId,
    pub swap_fee_bps: u128,
    pub credential_template: AlkaneId,
    pub quote: AlkaneId,
}

impl RegistryConfig {
    pub fn decode(data: &[u8]) -> Result<Self> {
        match unpack_u128s(data)?[..] {
            [creator_bps, supply, target, granularity, deadline_offset, lb, lt, ab, at, pb, pt, swap_fee_bps, cb, ct, qb, qt] => Ok(Self {
                creator_bps,
                supply,
                target,
                granularity,
                deadline_offset,
                launch_template: AlkaneId { block: lb, tx: lt },
                platform_auth: AlkaneId { block: ab, tx: at },
                pool_template: AlkaneId { block: pb, tx: pt },
                swap_fee_bps,
                credential_template: AlkaneId { block: cb, tx: ct },
                quote: AlkaneId { block: qb, tx: qt },
            }),
            _ => Err(anyhow!("unexpected registry config")),
        }
    }
}



pub fn clone_registry(source: &AlkaneId, config: &RegistryConfig) -> Cellpack {
    let mut inputs = vec![0];
    inputs.extend(id_words(&config.platform_auth));
    inputs.extend(id_words(&config.launch_template));
    inputs.extend([config.creator_bps, config.supply, config.target, config.granularity, config.deadline_offset]);
    inputs.extend(id_words(&config.pool_template));
    inputs.push(config.swap_fee_bps);
    inputs.extend(id_words(&config.credential_template));
    inputs.extend(id_words(&config.quote));
    Cellpack { target: AlkaneId { block: 5, tx: source.tx }, inputs }
}


pub fn without(parcel: &AlkaneTransferParcel, credential: &AlkaneId) -> Vec<AlkaneTransfer> {
    parcel.0.iter().filter(|t| t.id != *credential).cloned().collect()
}

#[derive(Default)]
pub struct MagaTrustee(());

#[derive(MessageDispatch)]
enum MagaTrusteeMessage {



    #[opcode(0)]
    Initialize { credential: AlkaneId, registries: Vec<AlkaneId> },

    #[opcode(1)]
    ClaimLaunchFee { registry: AlkaneId, project_id: u128 },

    #[opcode(2)]
    CollectPoolFees { registry: AlkaneId, project_id: u128 },

    #[opcode(7)]
    Deposit,


    #[opcode(20)]
    CreateRegistry { target: u128, granularity: u128, deadline_offset: u128 },

    #[opcode(90)]
    #[returns(Vec<u8>)]
    GetConfig,

    #[opcode(91)]
    #[returns(u128)]
    GetSealed,

    #[opcode(99)]
    #[returns(String)]
    GetName,
}



fn p_credential() -> StoragePointer {
    StoragePointer::from_keyword("/credential")
}


fn p_registries() -> StoragePointer {
    StoragePointer::from_keyword("/registries")
}

fn p_listed(registry: &AlkaneId) -> StoragePointer {
    StoragePointer::from_keyword("/listed/").select(&pack_id(registry))
}

#[derive(Clone, Copy)]
enum Claim {
    LaunchFee,
    PoolFees,
}

impl MagaTrustee {
    fn credential(&self) -> Result<AlkaneId> {
        let bytes = p_credential().get_bytes();
        if bytes.is_empty() {
            return Err(anyhow!("not initialized"));
        }
        unpack_id(&bytes)
    }

    fn registries(&self) -> Result<Vec<AlkaneId>> {
        p_registries().get_bytes().chunks(32).map(unpack_id).collect()
    }

    fn registry_config(&self, registry: &AlkaneId) -> Result<RegistryConfig> {
        RegistryConfig::decode(&self.staticcall(&get_config(registry), &AlkaneTransferParcel::default(), self.fuel())?.data)
    }

    fn initialize(&self, credential: AlkaneId, registries: Vec<AlkaneId>) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;
        if !context.incoming_alkanes.0.is_empty() {
            return Err(anyhow!("initialize takes no assets"));
        }
        let zero = AlkaneId::default();
        if credential == zero || credential == context.myself {
            return Err(anyhow!("invalid credential"));
        }
        if registries.is_empty() {
            return Err(anyhow!("at least one registry"));
        }
        for (i, listed) in registries.iter().enumerate() {
            if *listed == zero || *listed == credential || *listed == context.myself {
                return Err(anyhow!("invalid registry"));
            }
            if registries[..i].contains(listed) {
                return Err(anyhow!("registry listed twice"));
            }
        }


        let master = registries[0];
        if master.block != 2 {
            return Err(anyhow!("the master registry must be a contract at 2:<tx>"));
        }
        if self.registry_config(&master)?.platform_auth != credential {
            return Err(anyhow!("the master registry uses another platform credential"));
        }
        p_credential().set_bytes(pack_id(&credential));
        p_registries().set_bytes(registries.iter().flat_map(pack_id).collect());
        for listed in &registries {
            p_listed(listed).set_value::<u8>(1);
        }

        let mut response = CallResponse::default();
        response.alkanes.0.push(AlkaneTransfer { id: context.myself, value: 1 });
        Ok(response)
    }



    fn deposit(&self) -> Result<CallResponse> {
        let credential = self.credential()?;
        let context = self.context()?;
        match &context.incoming_alkanes.0[..] {
            [t] if t.id == credential && t.value > 0 => Ok(CallResponse::default()),
            _ => Err(anyhow!("deposit takes the platform credential alone")),
        }
    }



    fn require_revenue_token(&self, context: &alkanes_support::context::Context) -> Result<()> {
        match &context.incoming_alkanes.0[..] {
            [t] if t.id == context.myself && t.value > 0 => Ok(()),
            _ => Err(anyhow!("requires the revenue token alone")),
        }
    }






    fn create_registry(&self, target: u128, granularity: u128, deadline_offset: u128) -> Result<CallResponse> {
        self.credential()?;
        let context = self.context()?;
        self.require_revenue_token(&context)?;
        let mut registries = self.registries()?;
        let master = registries[0];
        let config = RegistryConfig { target, granularity, deadline_offset, ..self.registry_config(&master)? };




        let registry = AlkaneId { block: 2, tx: self.sequence() };
        self.call(&clone_registry(&master, &config), &AlkaneTransferParcel::default(), self.fuel())?;
        if self.registry_config(&registry)? != config {
            return Err(anyhow!("the clone is not at the predicted id"));
        }
        registries.push(registry);
        p_registries().set_bytes(registries.iter().flat_map(pack_id).collect());
        p_listed(&registry).set_value::<u8>(1);

        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = pack_u128s(&id_words(&registry));
        Ok(response)
    }




    fn claim(&self, registry: AlkaneId, project_id: u128, claim: Claim) -> Result<CallResponse> {
        let credential = self.credential()?;
        let context = self.context()?;


        self.require_revenue_token(&context)?;


        if p_listed(&registry).get().is_empty() {
            return Err(anyhow!("unknown registry"));
        }
        let record = self.staticcall(&get_project(&registry, project_id), &AlkaneTransferParcel::default(), self.fuel())?;
        let (launch, pool) = decode_project(&record.data, project_id)?;



        let sealed = self.balance(&context.myself, &credential);
        if sealed == 0 {
            return Err(anyhow!("no credential deposited"));
        }
        let call = match claim {
            Claim::LaunchFee => claim_platform_fee(&launch),
            Claim::PoolFees => collect_fees(&pool),
        };
        let paid = self.call(
            &call,
            &AlkaneTransferParcel(vec![AlkaneTransfer { id: credential, value: 1 }]),
            self.fuel(),
        )?;



        if self.balance(&context.myself, &credential) != sealed {
            return Err(anyhow!("the credential did not come back"));
        }
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.alkanes.0.extend(without(&paid.alkanes, &credential));
        response.data = paid.data;
        Ok(response)
    }

    fn claim_launch_fee(&self, registry: AlkaneId, project_id: u128) -> Result<CallResponse> {
        self.claim(registry, project_id, Claim::LaunchFee)
    }

    fn collect_pool_fees(&self, registry: AlkaneId, project_id: u128) -> Result<CallResponse> {
        self.claim(registry, project_id, Claim::PoolFees)
    }


    fn get_config(&self) -> Result<CallResponse> {
        let registries = self.registries()?;
        let mut data = id_words(&self.credential()?);
        data.push(registries.len() as u128);
        data.extend(registries.iter().flat_map(id_words));
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);
        response.data = pack_u128s(&data);
        Ok(response)
    }

    fn get_sealed(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = self.balance(&context.myself, &self.credential()?).to_le_bytes().to_vec();
        Ok(response)
    }

    fn get_name(&self) -> Result<CallResponse> {
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);
        response.data = b"MAGA Trustee".to_vec();
        Ok(response)
    }
}

impl AlkaneResponder for MagaTrustee {}

declare_alkane! {
    impl AlkaneResponder for MagaTrustee {
        type Message = MagaTrusteeMessage;
    }
}
