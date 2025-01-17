// SPDX-License-Identifier: MIT
//
// Copyright IBM Corp. 2023

#![cfg(test)]

use super::{helper, helper::*, *};
use crate::{Error, HkdVerifyErrorType::*};
use core::slice;
use once_cell::sync::OnceCell;
use openssl::stack::Stack;
use std::sync::Mutex;

use crate::test_utils::*;

pub fn mock_endpt(res: &str) -> mockito::Mock {
    static MOCK_SERVER: OnceCell<Mutex<mockito::Server>> = OnceCell::new();

    let res_path = get_cert_asset_path(res);

    MOCK_SERVER
        .get_or_init(|| mockito::Server::new_with_port(1234).into())
        .lock()
        .expect("COULD NOT GET THE MOCK_SERVER LOCK")
        .mock("GET", format!("/crl/{res}").as_str())
        .with_header("content-type", "application/pkix-crl")
        .with_body_from_file(res_path)
        .create()
}

#[test]
fn mockito_server_available() {
    let _mock = mock_endpt("ibm.crt");
}

#[test]
fn store_setup() {
    let ibm_str = get_cert_asset_path_string("ibm.crt");
    let inter_str = get_cert_asset_path_string("inter.crt");

    let store = helper::store_setup(&None, &[], &[ibm_str, inter_str]);
    assert!(store.is_ok());
}

#[test]
fn verify_chain_online() {
    let ibm_crt = load_gen_cert("ibm.crt");
    let inter_crt = load_gen_cert("inter_ca.crt");
    let root_crt = get_cert_asset_path_string("root_ca.chained.crt");

    let mock_inter = mock_endpt("inter_ca.crl");

    let mut store = helper::store_setup(&Some(root_crt), &[], &[]).unwrap();
    download_crls_into_store(&mut store, slice::from_ref(&ibm_crt)).unwrap();
    let store = store.build();

    mock_inter.assert();

    let mut sk = Stack::<X509>::new().unwrap();
    sk.push(inter_crt).unwrap();
    verify_chain(&store, &sk, &[ibm_crt.clone()]).unwrap();
    assert!(verify_chain(&store, &sk, &[ibm_crt]).is_ok());
}

#[test]
fn verify_chain_offline() {
    let ibm_crt = load_gen_cert("ibm.crt");
    let inter_crl = get_cert_asset_path_string("inter_ca.crl");
    let inter_crt = load_gen_cert("inter_ca.crt");
    let root_crt = get_cert_asset_path_string("root_ca.chained.crt");

    let store = helper::store_setup(&Some(root_crt), &[inter_crl], &[])
        .unwrap()
        .build();

    let mut sk = Stack::<X509>::new().unwrap();
    sk.push(inter_crt).unwrap();
    assert!(verify_chain(&store, &sk, &[ibm_crt]).is_ok());
}

#[test]
fn verify_online() {
    let root_crt = get_cert_asset_path_string("root_ca.chained.crt");
    let inter_crt = get_cert_asset_path_string("inter_ca.crt");
    let ibm_crt = get_cert_asset_path_string("ibm.crt");
    let hkd_revoked = load_gen_cert("host_rev.crt");
    let hkd_inv = load_gen_cert("host_invalid_signing_key.crt");
    let hkd_exp = load_gen_cert("host_crt_expired.crt");
    let hkd = load_gen_cert("host.crt");

    let mock_inter = mock_endpt("inter_ca.crl");
    let mock_ibm = mock_endpt("ibm.crl");

    let inter_crl = get_cert_asset_path_string("inter_ca.crl");
    let ibm_crl = get_cert_asset_path_string("ibm.crl");
    let verifier = CertVerifier::new(
        &[ibm_crt, inter_crt],
        &[ibm_crl, inter_crl],
        &Some(root_crt),
        false,
    )
    .unwrap();

    mock_inter.assert();

    verifier.verify(&hkd).unwrap();

    mock_ibm.assert();
    assert!(matches!(
        verifier.verify(&hkd_revoked),
        Err(Error::HkdVerify(HdkRevoked))
    ));

    assert!(matches!(
        verifier.verify(&hkd_inv),
        Err(Error::HkdVerify(IssuerMismatch))
    ));

    assert!(matches!(
        verifier.verify(&hkd_exp),
        Err(Error::HkdVerify(AfterValidity))
    ));
}

#[test]
fn verify_offline() {
    let root_crt = get_cert_asset_path_string("root_ca.chained.crt");
    let inter_crt = get_cert_asset_path_string("inter_ca.crt");
    let inter_crl = get_cert_asset_path_string("inter_ca.crl");
    let ibm_crt = get_cert_asset_path_string("ibm.crt");
    let ibm_crl = get_cert_asset_path_string("ibm.crl");
    let hkd_revoked = load_gen_cert("host_rev.crt");
    let hkd_inv = load_gen_cert("host_invalid_signing_key.crt");
    let hkd_exp = load_gen_cert("host_crt_expired.crt");
    let hkd = load_gen_cert("host.crt");

    let verifier = CertVerifier::new(
        &[ibm_crt, inter_crt],
        &[ibm_crl, inter_crl],
        &Some(root_crt),
        true,
    )
    .unwrap();

    verifier.verify(&hkd).unwrap();
    assert!(matches!(
        verifier.verify(&hkd_revoked),
        Err(Error::HkdVerify(HdkRevoked))
    ));

    assert!(matches!(
        verifier.verify(&hkd_inv),
        Err(Error::HkdVerify(IssuerMismatch))
    ));

    assert!(matches!(
        verifier.verify(&hkd_exp),
        Err(Error::HkdVerify(AfterValidity))
    ));
}

#[test]
fn dist_points() {
    let crt = load_gen_cert("ibm.crt");
    let res = x509_dist_points(&crt);
    let exp = vec!["http://127.0.0.1:1234/crl/inter_ca.crl"];
    assert_eq!(res, exp);
}
