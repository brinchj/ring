// Copyright 2015-2016 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND AND THE AUTHORS DISCLAIM ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

/// RSA PKCS#1 1.5 signatures.

use {c, core, der, digest, error};
use untrusted;


pub struct RSAPadding {
    digest_alg: &'static digest::Algorithm,
    digestinfo_prefix: &'static [u8],
}

macro_rules! rsa_pkcs1_padding {
    ( $PADDING_ALGORITHM:ident, $digest_alg:expr, $digestinfo_prefix:expr,
      $doc_str:expr ) => {
        #[doc=$doc_str]
        /// Feature: `rsa_signing`.
        pub static $PADDING_ALGORITHM: RSAPadding = RSAPadding {
            digest_alg: $digest_alg,
            digestinfo_prefix: $digestinfo_prefix,
        };
    }
}

rsa_pkcs1_padding!(RSA_PKCS1_SHA1, &digest::SHA1,
                   &SHA1_PKCS1_DIGESTINFO_PREFIX,
                   "Signing using RSA with PKCS#1 1.5 padding and SHA-1.");
rsa_pkcs1_padding!(RSA_PKCS1_SHA256, &digest::SHA256,
                   &SHA256_PKCS1_DIGESTINFO_PREFIX,
                   "Signing using RSA with PKCS#1 1.5 padding and SHA-256.");
rsa_pkcs1_padding!(RSA_PKCS1_SHA384, &digest::SHA384,
                   &SHA384_PKCS1_DIGESTINFO_PREFIX,
                   "Signing using RSA with PKCS#1 1.5 padding and SHA3846.");
rsa_pkcs1_padding!(RSA_PKCS1_SHA512, &digest::SHA512,
                   &SHA512_PKCS1_DIGESTINFO_PREFIX,
                   "Signing using RSA with PKCS#1 1.5 padding and SHA-512.");

macro_rules! pkcs1_digestinfo_prefix {
    ( $name:ident, $digest_len:expr, $digest_oid_len:expr,
      [ $( $digest_oid:expr ),* ] ) => {
        static $name: [u8; 2 + 8 + $digest_oid_len] = [
            der::Tag::Sequence as u8, 8 + $digest_oid_len + $digest_len,
                der::Tag::Sequence as u8, 2 + $digest_oid_len + 2,
                    der::Tag::OID as u8, $digest_oid_len, $( $digest_oid ),*,
                    der::Tag::Null as u8, 0,
                der::Tag::OctetString as u8, $digest_len,
        ];
    }
}

macro_rules! pkcs1_digestinfo_prefix {
    ( $name:ident, $digest_len:expr, $digest_oid_len:expr,
      [ $( $digest_oid:expr ),* ] ) => {
        static $name: [u8; 2 + 8 + $digest_oid_len] = [
            der::Tag::Sequence as u8, 8 + $digest_oid_len + $digest_len,
                der::Tag::Sequence as u8, 2 + $digest_oid_len + 2,
                    der::Tag::OID as u8, $digest_oid_len, $( $digest_oid ),*,
                    der::Tag::Null as u8, 0,
                der::Tag::OctetString as u8, $digest_len,
        ];
    }
}

pkcs1_digestinfo_prefix!(
    SHA1_PKCS1_DIGESTINFO_PREFIX, 20, 5, [ 0x2b, 0x0e, 0x03, 0x02, 0x1a ]);

pkcs1_digestinfo_prefix!(
    SHA256_PKCS1_DIGESTINFO_PREFIX, 32, 9,
    [ 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01 ]);

pkcs1_digestinfo_prefix!(
    SHA384_PKCS1_DIGESTINFO_PREFIX, 48, 9,
    [ 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x02 ]);

pkcs1_digestinfo_prefix!(
    SHA512_PKCS1_DIGESTINFO_PREFIX, 64, 9,
    [ 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x03 ]);


/// Parameters for RSA signing and verification.
pub struct RSAParameters {
    padding_alg: &'static RSAPadding,
    min_bits: usize,
}

fn parse_public_key(input: untrusted::Input)
                    -> Result<(untrusted::Input, untrusted::Input),
                              error::Unspecified> {
    input.read_all(error::Unspecified, |input| {
        der::nested(input, der::Tag::Sequence, error::Unspecified, |input| {
            let n = try!(der::positive_integer(input));
            let e = try!(der::positive_integer(input));
            Ok((n, e))
        })
    })
}

struct PositiveInteger {
    value: Option<*mut BIGNUM>,
}

#[allow(unsafe_code)]
impl PositiveInteger {
    #[cfg(feature = "rsa_signing")]
    // Parses a single ASN.1 DER-encoded `Integer`, which most be positive.
    fn from_der(input: &mut untrusted::Reader)
                -> Result<PositiveInteger, error::Unspecified> {
        Self::from_be_bytes(try!(der::positive_integer(input)))
    }

    // Turns a sequence of big-endian bytes into a Positive Integer.
    fn from_be_bytes(input: untrusted::Input)
                     -> Result<PositiveInteger, error::Unspecified> {
        // Reject empty inputs.
        if input.len() == 0 {
            return Err(error::Unspecified);
        }
        // Reject leading zeros. Also reject the value zero ([0]) because zero
        // isn't positive.
        if untrusted::Reader::new(input).peek(0) {
            return Err(error::Unspecified);
        }
        let res = unsafe {
            GFp_BN_bin2bn(input.as_slice_less_safe().as_ptr(), input.len(),
                          core::ptr::null_mut())
        };
        if res.is_null() {
            return Err(error::Unspecified);
        }
        Ok(PositiveInteger { value: Some(res) })
    }

    unsafe fn as_ref<'a>(&'a self) -> &'a BIGNUM { &*self.value.unwrap() }

    #[cfg(feature = "rsa_signing")]
    fn into_raw(&mut self) -> *mut BIGNUM {
        let res = self.value.unwrap();
        self.value = None;
        res
    }
}

#[allow(unsafe_code)]
impl Drop for PositiveInteger {
    fn drop(&mut self) {
        match self.value {
            Some(val) => unsafe {
                GFp_BN_free(val);
            },
            None => {},
        }
    }
}

#[cfg(feature = "rsa_signing")]
#[allow(non_camel_case_types)]
enum BN_MONT_CTX {}


pub mod verification;

enum BIGNUM {}

#[cfg(feature = "rsa_signing")]
pub mod signing;


extern {
    fn GFp_BN_bin2bn(in_: *const u8, len: c::size_t, ret: *mut BIGNUM)
                     -> *mut BIGNUM;
    fn GFp_BN_free(bn: *mut BIGNUM);
}

#[cfg(feature = "rsa_signing")]
extern {
    fn GFp_BN_MONT_CTX_free(mont: *mut BN_MONT_CTX);
}


#[cfg(test)]
mod tests {
    use super::PositiveInteger;
    use untrusted;

    #[test]
    fn test_positive_integer_from_be_bytes_empty() {
        // Empty values are rejected.
        assert!(PositiveInteger::from_be_bytes(
                    untrusted::Input::from(&[])).is_err());
    }

    #[test]
    fn test_positive_integer_from_be_bytes_zero() {
        // The zero value is rejected.
        assert!(PositiveInteger::from_be_bytes(
                    untrusted::Input::from(&[0])).is_err());
        // A zero with a leading zero is rejected.
        assert!(PositiveInteger::from_be_bytes(
                    untrusted::Input::from(&[0, 0])).is_err());
        // A non-zero value with a leading zero is rejected.
        assert!(PositiveInteger::from_be_bytes(
                    untrusted::Input::from(&[0, 1])).is_err());
        // A non-zero value with no leading zeros is accepted.
        assert!(PositiveInteger::from_be_bytes(
                    untrusted::Input::from(&[1])).is_ok());
        // A non-zero value with that ends in a zero byte is accepted.
        assert!(PositiveInteger::from_be_bytes(
                    untrusted::Input::from(&[1, 0])).is_ok());
    }
}