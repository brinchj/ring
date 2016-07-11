/* Copyright 2016 Brian Smith.
 *
 * Permission to use, copy, modify, and/or distribute this software for any
 * purpose with or without fee is hereby granted, provided that the above
 * copyright notice and this permission notice appear in all copies.
 *
 * THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHORS DISCLAIM ALL WARRANTIES
 * WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY
 * SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 * WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
 * OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
 * CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE. */

#include "gfp_internal.h"

#include <string.h>

#include "../bn/internal.h"
#include "../internal.h"


typedef GFp_Limb Elem[P384_LIMBS];
typedef GFp_Limb ScalarMont[P384_LIMBS];
typedef GFp_Limb Scalar[P384_LIMBS];


/* Prototypes to avoid -Wmissing-prototypes warnings. */
void GFp_p384_elem_add(Elem r, const Elem a, const Elem b);
void GFp_p384_elem_mul_mont(Elem r, const Elem a, const Elem b);
void GFp_p384_scalar_inv_to_mont(ScalarMont r, const Scalar a);
void GFp_p384_scalar_mul_mont(ScalarMont r, const ScalarMont a,
                              const ScalarMont b);


static const BN_ULONG Q[P384_LIMBS] = {
  TOBN(0x00000000, 0xffffffff),
  TOBN(0xffffffff, 0x00000000),
  TOBN(0xffffffff, 0xfffffffe),
  TOBN(0xffffffff, 0xffffffff),
  TOBN(0xffffffff, 0xffffffff),
  TOBN(0xffffffff, 0xffffffff),
};

static const BN_ULONG N[P384_LIMBS] = {
  TOBN(0xecec196a, 0xccc52973),
  TOBN(0x581a0db2, 0x48b0a77a),
  TOBN(0xc7634d81, 0xf4372ddf),
  TOBN(0xffffffff, 0xffffffff),
  TOBN(0xffffffff, 0xffffffff),
  TOBN(0xffffffff, 0xffffffff),
};

OPENSSL_COMPILE_ASSERT(sizeof(size_t) == sizeof(GFp_Limb),
                       size_t_and_gfp_limb_are_different_sizes);

OPENSSL_COMPILE_ASSERT(sizeof(size_t) == sizeof(BN_ULONG),
                       size_t_and_bn_ulong_are_different_sizes);


/* XXX: MSVC for x86 warns when it fails to inline these functions it should
 * probably inline. */
#if defined(_MSC_VER)  && defined(OPENSSL_X86)
#define INLINE_IF_POSSIBLE __forceinline
#else
#define INLINE_IF_POSSIBLE inline
#endif


static INLINE_IF_POSSIBLE void copy_conditional(Elem r, const Elem a,
                                                const GFp_Limb condition) {
  for (size_t i = 0; i < P384_LIMBS; ++i) {
    r[i] = constant_time_select_size_t(condition, a[i], r[i]);
  }
}


static inline void elem_mul_mont(Elem r, const Elem a, const Elem b) {
  static const BN_ULONG Q_N0[] = {
    BN_MONT_CTX_N0(0x1, 0x1)
  };
  /* XXX: Not (clearly) constant-time; inefficient. TODO: Add a dedicated
   * squaring routine. */
  bn_mul_mont(r, a, b, Q, Q_N0, P384_LIMBS);
}

void GFp_p384_elem_add(Elem r, const Elem a, const Elem b) {
  /* XXX: Not constant-time. */
  if (!bn_add_words(r, a, b, P384_LIMBS)) {
    if (bn_cmp_words(r, Q, P384_LIMBS) < 0) {
      return;
    }
  }
  /* Either the addition resulted in a carry requiring 1 bit more than would
   * fit in |P384_LIMBS| limbs, or the addition result fit in |P384_LIMBS|
   * limbs but it was not less than |q|. Either way, it needs to be reduced. */
  (void)bn_sub_words(r, r, Q, P384_LIMBS);
}

void GFp_p384_elem_mul_mont(Elem r, const Elem a, const Elem b) {
  elem_mul_mont(r, a, b);
}


void GFp_p384_scalar_mul_mont(ScalarMont r, const ScalarMont a,
                              const ScalarMont b) {
  static const BN_ULONG N_N0[] = {
    BN_MONT_CTX_N0(0x6ed46089, 0xe88fdc45)
  };
  /* XXX: Inefficient. TODO: Add dedicated multiplication routine. */
  bn_mul_mont(r, a, b, N, N_N0, P384_LIMBS);
}