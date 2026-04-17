<template>
  <!-- This HTML comment should be removed -->
  <div class="app">
    <p>Hello Vue!</p>
    <!-- TODO: add dynamic content -->
  </div>
</template>

<script>
// This line comment should be removed
/* This block comment should be removed */
export default {
  name: "App",
  data() {
    const url = "http://example.com#anchor";
    const note = "// not a comment";
    // eslint-disable-next-line no-unused-vars
    return { url, note };
  },
};
</script>

<style scoped>
/* This CSS comment should be removed */
.app {
  color: red;
}
</style>
