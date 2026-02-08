// Copyright 2018 Mozilla
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

#![allow(dead_code)]

use std;

use futures::executor::block_on;
use reqwest::{StatusCode, Url};
// TODO: enable TLS support; hurdle is cross-compiling openssl for Android.
// See https://github.com/mozilla/mentat/issues/569
// use hyper_tls;
// TODO: https://github.com/mozilla/mentat/issues/570
// use serde_cbor;
use serde_json;
use uuid::Uuid;

use crate::{logger, GlobalTransactionLog, Tx, TxPart};
use logger::d;
use public_traits::errors::Result;

#[derive(Serialize, Deserialize)]
struct SerializedHead {
    head: Uuid,
}

#[derive(Serialize)]
struct SerializedTransaction<'a> {
    parent: &'a Uuid,
    chunks: &'a Vec<Uuid>,
}

#[derive(Deserialize)]
struct DeserializableTransaction {
    parent: Uuid,
    chunks: Vec<Uuid>,
    id: Uuid,
    seq: i64,
}

#[derive(Deserialize)]
struct SerializedTransactions {
    limit: i64,
    from: Uuid,
    transactions: Vec<Uuid>,
}

pub struct RemoteClient {
    client: reqwest::Client,
    base_uri: String,
    user_uuid: Uuid,
}

impl RemoteClient {
    pub fn new(base_uri: String, user_uuid: Uuid) -> Self {
        RemoteClient {
            client: reqwest::Client::new(),
            base_uri,
            user_uuid,
        }
    }

    fn bound_base_uri(&self) -> String {
        // TODO escaping
        format!("{}/{}", self.base_uri, self.user_uuid)
    }

    // TODO what we want is a method that returns a deserialized json structure.
    // It'll need a type T so that consumers can specify what downloaded json will
    // map to. I ran into borrow issues doing that - probably need to restructure
    // this and use PhantomData markers or somesuch.
    // But for now, we get code duplication.
    async fn get_uuid(&self, uri: String) -> Result<Uuid> {
        // TODO https://github.com/mozilla/mentat/issues/569
        // let client = hyper::Client::configure()
        //     .connector(hyper_tls::HttpsConnector::new(4, &core.handle()).unwrap())
        //     .build(&core.handle());
        d(&format!("client"));
        let uri =
            Url::parse(&uri).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        d(&format!("parsed uri {:?}", uri));
        let head_json: SerializedHead = self
            .client
            .get(uri)
            .send()
            .await?
            .json()
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        d(&format!("got head: {:?}", &head_json.head));
        Ok(head_json.head)
    }

    async fn put<T: serde::Serialize>(
        &self,
        uri: String,
        payload: T,
        expected: StatusCode,
    ) -> Result<()> {
        d(&format!("PUT {:?}", uri));

        let response = self
            .client
            .put(uri)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                d(&format!("error sending PUT request: {:?}", e));
                std::io::Error::new(std::io::ErrorKind::Other, e)
            })?;

        let status_code = response.status();

        if status_code != expected {
            d(&format!("bad put response: {:?}", status_code));
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("expected {:?}, got {:?}", expected, status_code),
            ))?;
        }

        Ok(())
    }

    async fn get_transactions(&self, parent_uuid: &Uuid) -> Result<Vec<Uuid>> {
        d(&format!("client"));

        let uri = format!(
            "{}/transactions?from={}",
            self.bound_base_uri(),
            parent_uuid
        );

        let response = self.client.get(uri).send().await?;
        println!("Response: {}", response.status());

        let body = response.bytes().await?;
        let json: SerializedTransactions = serde_json::from_slice(&body)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        d(&format!("got transactions: {:?}", &json.transactions));
        Ok(json.transactions)
    }

    async fn get_chunks(&self, transaction_uuid: &Uuid) -> Result<Vec<Uuid>> {
        let uri = format!(
            "{}/transactions/{}",
            self.bound_base_uri(),
            transaction_uuid
        );

        let response = self.client.get(uri).send().await?;
        println!("Response: {}", response.status());

        let body = response.bytes().await?;
        let json: DeserializableTransaction = serde_json::from_slice(&body)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        d(&format!("got transaction chunks: {:?}", &json.chunks));
        Ok(json.chunks)
    }

    async fn get_chunk(&self, chunk_uuid: &Uuid) -> Result<TxPart> {
        // TODO https://github.com/mozilla/mentat/issues/569
        // let client = hyper::Client::configure()
        //     .connector(hyper_tls::HttpsConnector::new(4, &core.handle()).unwrap())
        //     .build(&core.handle());
        d(&format!("client"));

        let uri = format!("{}/chunks/{}", self.bound_base_uri(), chunk_uuid);

        let response = self.client.get(uri).send().await?;
        let body = response.bytes().await?;
        let chunk: TxPart = serde_json::from_slice(&body)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        d(&format!("got chunk: {:?}", &chunk));
        d(&format!("got transaction chunk: {:?}", &chunk));
        Ok(chunk)
    }
}

impl GlobalTransactionLog for RemoteClient {
    fn head(&self) -> Result<Uuid> {
        let uri = format!("{}/head", self.bound_base_uri());
        block_on(self.get_uuid(uri))
    }

    fn set_head(&mut self, uuid: &Uuid) -> Result<()> {
        // {"head": uuid}
        let head = SerializedHead { head: uuid.clone() };

        let uri = format!("{}/head", self.bound_base_uri());
        let json = serde_json::to_string(&head)?;
        d(&format!("serialized head: {:?}", json));
        block_on(self.put(uri, json, StatusCode::NO_CONTENT))
    }

    /// Slurp transactions and datoms after `tx`, returning them as owned data.
    ///
    /// This is inefficient but convenient for development.
    fn transactions_after(&self, tx: &Uuid) -> Result<Vec<Tx>> {
        let new_txs = block_on(self.get_transactions(tx))?;
        let mut tx_list = Vec::new();

        for tx in new_txs {
            let mut tx_parts = Vec::new();
            let chunks = block_on(self.get_chunks(&tx))?;

            // We pass along all of the downloaded parts, including transaction's
            // metadata datom. Transactor is expected to do the right thing, and
            // use txInstant from one of our datoms.
            for chunk in chunks {
                let part = block_on(self.get_chunk(&chunk))?;
                tx_parts.push(part);
            }

            tx_list.push(Tx {
                tx: tx.into(),
                parts: tx_parts,
            });
        }

        d(&format!("got tx list: {:?}", &tx_list));

        Ok(tx_list)
    }

    fn put_transaction(
        &mut self,
        transaction_uuid: &Uuid,
        parent_uuid: &Uuid,
        chunks: &Vec<Uuid>,
    ) -> Result<()> {
        // {"parent": uuid, "chunks": [chunk1, chunk2...]}
        let transaction = SerializedTransaction {
            parent: parent_uuid,
            chunks,
        };

        let uri = format!(
            "{}/transactions/{}",
            self.bound_base_uri(),
            transaction_uuid
        );
        let json = serde_json::to_string(&transaction)?;
        d(&format!("serialized transaction: {:?}", json));
        block_on(
            self.client
                .put(uri)
                .header("Content-Type", "application/json")
                .json(&transaction)
                .send(),
        )
        .map_err(|e| {
            d(&format!("error sending PUT request: {:?}", e));
            std::io::Error::new(std::io::ErrorKind::Other, e)
        })?;

        Ok(())
    }

    fn put_chunk(&mut self, chunk_uuid: &Uuid, payload: &TxPart) -> Result<()> {
        let payload: String = serde_json::to_string(payload)?;
        let uri = format!("{}/chunks/{}", self.bound_base_uri(), chunk_uuid);
        d(&format!("serialized chunk: {:?}", payload));
        // TODO don't want to clone every datom!
        block_on(self.put(uri, payload, StatusCode::CREATED))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_remote_client_bound_uri() {
        let user_uuid = Uuid::from_str(&"316ea470-ce35-4adf-9c61-e0de6e289c59").expect("uuid");
        let server_uri = String::from("https://example.com/api/0.1");
        let remote_client = RemoteClient::new(server_uri, user_uuid);
        assert_eq!(
            "https://example.com/api/0.1/316ea470-ce35-4adf-9c61-e0de6e289c59",
            remote_client.bound_base_uri()
        );
    }
}
