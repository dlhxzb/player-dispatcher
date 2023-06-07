pub mod proto;

use tonic::{Response, Status};

pub type RPCResult<T> = Result<Response<T>, Status>;

pub trait MapErrUnknown {
    type S;
    fn map_err_unknown(self) -> std::result::Result<Self::S, Status>;
}

impl<T, E: std::fmt::Debug> MapErrUnknown for Result<T, E> {
    type S = T;
    fn map_err_unknown(self) -> Result<Self::S, Status> {
        self.map_err(|e| {
            let s = format!("{e:?}");
            log::error!("{}", s);
            Status::unknown(s)
        })
    }
}
