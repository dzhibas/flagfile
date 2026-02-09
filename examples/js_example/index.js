import { init, ff } from "flagfile-ts";

init();

const ctx = {
  tier: "premium",
  countryCode: "nl",
};

if (ff("FF-feature-y", ctx)) {
  console.log("Flag is on");
} else {
  console.log("Flag is off");
}

console.log(ff("FF-simple"));
console.log(ff("FF-log"));
console.log(ff("FF-version"));

console.log(ff("FF-contains-feature-check", { name: "jdsjhsdfjhfds NIK sdjsdjh"}))
console.log(ff("FF-regexp-feature-check", { name: "some other check ola demo"}))