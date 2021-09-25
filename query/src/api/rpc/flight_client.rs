// Copyright 2020 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::convert::TryInto;

use common_arrow::arrow_flight::flight_service_client::FlightServiceClient;
use common_arrow::arrow_flight::Action;
use common_arrow::arrow_flight::FlightData;
use common_arrow::arrow_flight::Ticket;
use common_datavalues::DataSchemaRef;
use common_exception::ErrorCode;
use common_exception::Result;
use common_base::tokio::time::Duration;
use common_streams::SendableDataBlockStream;
use tonic::transport::channel::Channel;
use tonic::Request;
use tonic::Streaming;

use crate::api::rpc::flight_actions::FlightAction;
use crate::api::rpc::flight_client_stream::FlightDataStream;
use crate::api::rpc::flight_tickets::FlightTicket;

pub struct FlightClient {
    inner: FlightServiceClient<Channel>,
}

// TODO: Integration testing required
impl FlightClient {
    pub fn new(inner: FlightServiceClient<Channel>) -> FlightClient {
        FlightClient { inner }
    }

    pub async fn fetch_stream(
        &mut self,
        ticket: FlightTicket,
        schema: DataSchemaRef,
        timeout: u64,
    ) -> Result<SendableDataBlockStream> {
        let ticket = ticket.try_into()?;
        let inner = self.do_get(ticket, timeout).await?;
        Ok(Box::pin(FlightDataStream::from_remote(schema, inner)))
    }

    pub async fn execute_action(&mut self, action: FlightAction, timeout: u64) -> Result<()> {
        self.do_action(action, timeout).await?;
        Ok(())
    }

    // Execute do_get.
    async fn do_get(&mut self, ticket: Ticket, timeout: u64) -> Result<Streaming<FlightData>> {
        let mut request = Request::new(ticket);
        request.set_timeout(Duration::from_secs(timeout));

        let response = self.inner.do_get(request).await?;
        Ok(response.into_inner())
    }

    // Execute do_action.
    async fn do_action(&mut self, action: FlightAction, timeout: u64) -> Result<Vec<u8>> {
        let action: Action = action.try_into()?;
        let action_type = action.r#type.clone();
        let mut request = Request::new(action);
        request.set_timeout(Duration::from_secs(timeout));

        let response = self.inner.do_action(request).await?;

        match response.into_inner().message().await? {
            Some(response) => Ok(response.body),
            None => Result::Err(ErrorCode::EmptyDataFromServer(format!(
                "Can not receive data from flight server, action: {:?}",
                action_type
            ))),
        }
    }
}
