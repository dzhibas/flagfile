import { init, ff, initWithEnv } from "flagfile-ts";

init();

console.log("=== Flagfile JS Example ===\n");

// Conditional flag with context
const ctx = {
  tier: "premium",
  countryCode: "nl",
};

console.log("FF-feature-y (countryCode=nl):", ff("FF-feature-y", ctx) ? "ON" : "OFF");

// Simple flags
console.log("FF-simple:", ff("FF-simple"));
console.log("FF-log:", ff("FF-log"));
console.log("FF-version:", ff("FF-version"));

// String contains and regex (~)
console.log("\n--- String matching ---");
console.log('FF-contains-feature-check (name="jdsjhsdfjhfds NIK sdjsdjh"):', ff("FF-contains-feature-check", { name: "jdsjhsdfjhfds NIK sdjsdjh"}));
console.log('FF-regexp-feature-check (name="some other check ola demo"):', ff("FF-regexp-feature-check", { name: "some other check ola demo"}));

// startsWith / endsWith (~$, ^~)
console.log("\n--- startsWith / endsWith ---");
console.log('FF-email-domain-check (email="nikolajus@tesla.com"):', ff("FF-email-domain-check", {email: "nikolajus@tesla.com"}));
console.log('FF-email-domain-check (email="NIKOLAJUS@other.com"):', ff("FF-email-domain-check", {email: "NIKOLAJUS@other.com"}));

console.log("\n--- segments and envs ---");
console.log("FF-feature-y-2(country=nl):", ff("FF-feature-y-2", { country: "nl" }));

console.log("\n--- array ---");
// TODO: fix this as its not working properly
console.log("FF-admin-panel(roles=[viewer, editor, admin]):", ff("FF-admin-panel", { roles: ["viewer", "editor", "admin"] }));