<template>
  <div class="track-grid track-grid-head" aria-hidden="true">
    <span class="track-col-index" :class="$style.number">#</span>
    <span />
    <span>{{ $t('table.title') }}</span>
    <span class="track-col-artist">{{ $t('table.artist') }}</span>
    <span class="track-col-album">{{ $t(kindColumn ? 'table.kind' : 'table.album') }}</span>
    <span class="track-col-source">{{ $t('table.source') }}</span>
    <span :class="$style.number">{{ $t('table.duration') }}</span>
    <span />
    <span />
  </div>
</template>

<script setup lang="ts">
defineProps<{ kindColumn?: boolean }>();
</script>

<style module lang="scss">
.number {
  text-align: right;
}
</style>
