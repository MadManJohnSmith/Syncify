<script setup lang="ts">
import ProtoHost from './ProtoHost.vue'
import type { ProtoNote } from './types'
import VariantBaseline from './VariantBaseline.vue'
import VariantCoverage from './VariantCoverage.vue'
import VariantLedger from './VariantLedger.vue'
import VariantHealth from './VariantHealth.vue'
import VariantDetail from './VariantDetail.vue'
import { PROTO_SERVICES, formatInt } from './accountsPrototypeData'

/**
 * PROTOTIPO DESECHABLE — /__prototype/accounts?variant=…
 * Datos simulados en `accountsPrototypeData.ts`, sin IPC ni persistencia.
 * Estas cinco variantes son anteriores a la pasada de unificación y **no** están
 * auditadas contra la API real: se migró el host, no el contenido.
 */

import { VARIANTS } from './accountsVariants'

const COMPONENTS = {
  actual: VariantBaseline,
  cobertura: VariantCoverage,
  libro: VariantLedger,
  salud: VariantHealth,
  detalle: VariantDetail,
}

const accounts = PROTO_SERVICES.flatMap(service => service.accounts)
const fragile = PROTO_SERVICES.filter(service => service.status === 'expiring' || service.status === 'invalid')
const connected = PROTO_SERVICES.filter(service => service.status === 'connected' || service.status === 'expiring')
const tracks = PROTO_SERVICES.reduce((sum, service) => sum + service.library.tracks, 0)

const NOTES: ProtoNote[] = [
  { label: 'Servicios', value: String(PROTO_SERVICES.length), formula: 'COUNT(DISTINCT service)', caveat: 'Un servicio con cuatro cuentas cuenta una vez; una cuenta no es un servicio.' },
  { label: 'Cuentas', value: String(accounts.length), formula: 'COUNT(*) FROM accounts', caveat: 'Varias cuentas del mismo servicio son legítimas: cuentas distintas, catálogos distintos.' },
  { label: 'Conectados', value: String(connected.length), formula: "status IN ('connected','expiring')", caveat: '«Expirando» sigue funcionando hoy; puede dejar de hacerlo mañana.' },
  { label: 'Requieren acción', value: String(fragile.length), formula: "status IN ('expiring','invalid')", caveat: 'Solo por estado: la cobertura baja no siempre es un problema de credenciales.' },
  { label: 'Pistas', value: formatInt(tracks), formula: 'SUM(library.tracks)', caveat: 'Suma por servicio: la misma pista en dos servicios cuenta dos veces.' },
  { label: 'Cobertura', value: '—', formula: 'imported ÷ reported', caveat: 'No se calcula aquí: depende de la última sincronización, que es otro dato.' },
]

const subtitle = `${PROTO_SERVICES.length} servicios · ${accounts.length} cuentas · ${fragile.length} requieren atención`
</script>

<template>
  <ProtoHost tab="Accounts" :subtitle="subtitle" :variants="VARIANTS" :components="COMPONENTS" :notes="NOTES" :default-variant="'actual'" />
</template>
