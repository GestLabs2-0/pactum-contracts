# Guía `pactum-core` (as implemented)

> **As implemented** — documenta el programa `programs/pactum-core` en `pactum-contracts` (`2u1JrVasLvuGR5s3n84p5yaitHU2PGa8VjWZ7P2Eescm`).
> **SoT técnica:** [pactum-programs-spec.md](../../docs/architecture/pactum-programs-spec.md).

`pactum-core` centraliza la identidad raíz (`UserMainAcc`), la creación *permissionless* de `Marketplace`s sin dueño, los perfiles aislados por rol (`Client`, `Provider`, `Judge`), el catálogo reutilizable de `Offer`s y el registro B2B de árbitros (`ArbiterApplication` / `ArbiterProfile`). Toda mutación cruzada de métricas, *stakes* o disputas exige firma PDA `cpi_authority` validada contra `GlobalConfig` mediante `assert_cpi_authority`.

## Camino rápido

1. **Admin:** `initialize_core_config` → crea `CoreConfig` enlazado a `GlobalConfig`.
2. **Usuario:** `init_user_main` → crea `UserMainAcc`; si el mercado exige identidad (`requires_identity == true`), enlaza su *attestation* con `link_sas_identity`.
3. **Mercado (permissionless):** `create_marketplace(market_id, params)` → define `committee`, `dispute_mode` (`ArbiterOnly` | `DecentralizedJury` | `Hybrid`), `requires_identity` y mínimos de *stake*/*fee*.
4. **Participantes:** `init_client`, `init_provider` o `init_judge` (este último bloqueado si `dispute_mode == ArbiterOnly`).
5. **Ofertas:** `create_offer(params)`, `update_offer(params)` y `close_offer` (devuelve renta exclusivamente a `offer.rent_payer`).
6. **Árbitros B2B:** `apply_as_arbiter(arbiter_fee, sla_secs)` → `review_arbiter_application(approve)` (comité crea `ArbiterProfile` solo si aprueba y cierra `ArbiterApplication` → `rent_payer`) → `revoke_arbiter`.

## Detalles

### Cuentas clave y seeds

| Cuenta | Seeds | Campos clave |
|--------|-------|--------------|
| `CoreConfig` | `["core_config"]` | `authority`, `global_config`, `bump` |
| `UserMainAcc` | `["user_main", authority]` | `authority`, `sas_attestation: Option<Pubkey>`, `is_banned`, `bump` |
| `Identity` | `["identity", id_hash]` | `verified_at`, `linked`, `bump` *(struct de compatibilidad)* |
| `Marketplace` | `["marketplace", market_id]` | `market_id`, `committee`, `dispute_mode`, `requires_identity`, `min_judge_stake`, `max_judge_stake`, `min_arbiter_stake`, `min_jury_fee`, `pool_count`, `is_active`, `bump` |
| `Client` | `["client", marketplace, user_main]` | `marketplace`, `user_main`, `is_banned`, `completed_agreements`, `cancelled_agreements`, `disputes_won`, `disputes_lost`, `total_reviews`, `total_score`, `bump` |
| `Provider` | `["provider", marketplace, user_main]` | `marketplace`, `user_main`, `is_banned`, `offer_count`, `completed_agreements`, `cancelled_agreements`, `disputes_won`, `disputes_lost`, `total_reviews`, `total_score`, `bump` |
| `Judge` | `["judge", marketplace, user_main]` | `marketplace`, `user_main`, `staked_amount`, `pool_id`, `leaf_index`, `is_active_in_pool`, `active_disputes`, `cases_voted`, `cases_in_consensus`, `is_banned`, `bump` |
| `Offer` | `["offer", provider, offer_id]` | `provider`, `marketplace`, `offer_id`, `base_price`, `pricing_type`, `approval_window_secs`, `provider_bond`, `required_client_bond`, `jury_fee`, `default_arbiter`, `content_ref`, `state_hash`, `is_active`, `rent_payer`, `bump` |
| `ArbiterApplication` | `["arbiter_app", marketplace, authority]` | `marketplace`, `authority`, `sas_kyb_attestation`, `arbiter_fee`, `sla_secs`, `rent_payer`, `bump` |
| `ArbiterProfile` | `["arbiter", marketplace, authority]` | `marketplace`, `authority`, `sas_kyb_attestation`, `arbiter_fee`, `staked_amount`, `sla_secs`, `active_disputes`, `cases_resolved`, `is_active`, `bump` |

*(Nota: `Provider` ya no guarda `deposited_bond` global; el bond opcional vive aislado por `Agreement` en escrow.)*

### Instrucciones públicas (`instructions/`)

| Categoría | Instrucción | Quién firma | Efecto |
|-----------|-------------|-------------|--------|
| **Identidad / Admin** | `initialize_core_config` | `global_config.authority` | Crea `CoreConfig` |
| | `init_user_main` | `authority`, `payer` | Crea `UserMainAcc` |
| | `link_sas_identity` | `authority` | Valida `sas_attestation.owner == global_config.sas_program` y guarda `sas_attestation` |
| | `set_global_ban(banned)` | `global_config.authority` | Actualiza `user_main.is_banned` |
| **Mercados / Roles** | `create_marketplace(market_id, params)` | `payer` *(permissionless)* | Crea `Marketplace` con su `committee` |
| | `init_client` | `authority`, `payer` | Crea `Client` (valida `!is_banned` y SAS si `requires_identity`) |
| | `init_provider` | `authority`, `payer` | Crea `Provider` (valida `!is_banned` y SAS si `requires_identity`) |
| | `init_judge` | `authority`, `payer` | Crea `Judge` (exige `dispute_mode != ArbiterOnly`) |
| | `set_local_ban(role, banned)` | `marketplace.committee` | Banea/desbanea `Client`, `Provider` o `Judge` en ese mercado |
| **Ofertas** | `create_offer(params)` | `authority`, `payer` | Crea `Offer` reutilizable, incrementa `provider.offer_count` y fija `rent_payer` |
| | `update_offer(params)` | `authority` | Actualiza precio, *bonds*, `default_arbiter`, hashes o `is_active` |
| | `close_offer` | `authority` | Cierra `Offer` devolviendo *lamports* únicamente a `offer.rent_payer` |
| **Árbitros B2B** | `apply_as_arbiter(arbiter_fee, sla_secs)` | `authority`, `payer` | Valida `sas_kyb_attestation` y crea `ArbiterApplication` con `rent_payer` |
| | `review_arbiter_application(approve)` | `marketplace.committee` | Si aprueba crea `ArbiterProfile`; cierra `ArbiterApplication` → `rent_payer` |
| | `revoke_arbiter` | `marketplace.committee` | Marca `arbiter_profile.is_active = false` |

### Mutadores CPI privilegiados (`instructions/cpi/`)

Requieren `cpi_authority` (PDA del programa llamador) validada con `assert_cpi_authority`:

| Instrucción | Caller permitido | Efecto |
|-------------|------------------|--------|
| `cpi_update_judge_stake(amount, is_stake, pool_id, leaf_index)` | `Treasury` | Actualiza `staked_amount`, `pool_id`, `leaf_index`, `is_active_in_pool` |
| `cpi_update_arbiter_stake(amount, is_stake)` | `Treasury` | Actualiza `arbiter_profile.staked_amount` |
| `cpi_record_agreement_outcome(role, outcome)` | `Escrow` | Incrementa `completed_agreements` o `cancelled_agreements` en `Client`/`Provider` |
| `cpi_update_role_review(role, score)` | `Escrow` | Valida `1..=5` y acumula `total_reviews` / `total_score` en `Client`/`Provider` |
| `cpi_update_active_disputes(target_role, delta)` | `Disputes` | Ajusta `active_disputes` en `Judge` o `ArbiterProfile` |
| `cpi_record_dispute_result(winner)` | `Disputes` | Actualiza `disputes_won` y `disputes_lost` en `Client` y `Provider` |
| `cpi_record_judge_vote(in_consensus)` | `Disputes` | Incrementa `cases_voted` (y `cases_in_consensus` si aplica) |
| `cpi_record_arbiter_resolution` | `Disputes` | Incrementa `arbiter_profile.cases_resolved` |

### Verificación

- Tests atomizados por instrucción en `programs/pactum-core/tests/` (LiteSVM): **21/21 pasando** (`1` unit test + `20` tests de integración).
- `cargo fmt -p pactum-core -- --check` y `cargo clippy -p pactum-core --all-targets -- -Dwarnings` con **0 warnings**.

## Checklist

- [x] Una instrucción por archivo en `instructions/` y CPIs aislados en `instructions/cpi/`
- [x] *Seeds* documentadas sobre cada struct `#[account]` en `state/`
- [x] Devolución estricta de renta a `rent_payer` en `close_offer` y `review_arbiter_application`
- [x] Tests atómicos por instrucción en LiteSVM (`21/21` pasando)

## Próximo paso

Migrar `pactum-config` (completar `protocol_fee_bps`, `max_protocol_fee`, `dispute_escalation_fee` y `update_config`) y continuar con `pactum-escrow`.
