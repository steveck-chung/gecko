/* import-globals-from ../../../../../testing/mochitest/tests/SimpleTest/SimpleTest.js */
/* import-globals-from ../../../../../toolkit/components/satchel/test/satchel_common.js */
/* eslint-disable no-unused-vars */

"use strict";

let formFillChromeScript;

const VALID_ADDRESS_FIELDS = [
  "given-name",
  "additional-name",
  "family-name",
  "organization",
  "street-address",
  "address-level2",
  "address-level1",
  "postal-code",
  "country",
  "tel",
  "email",
];

function setInput(selector, value) {
  let input = document.querySelector("input" + selector);
  input.value = value;
  input.focus();
}

function clickOnElement(selector) {
  let element = document.querySelector(selector);

  if (!element) {
    throw new Error("Can not find the element");
  }

  element.click();
}

function onAddressChanged(type) {
  return new Promise(resolve => {
    formFillChromeScript.addMessageListener("formautofill-storage-changed", function onChanged(data) {
      formFillChromeScript.removeMessageListener("formautofill-storage-changed", onChanged);
      is(data.data, type, "Receive add storage changed event");
      resolve();
    });
  });
}

function checkMenuEntries(expectedValues) {
  let actualValues = getMenuEntries();

  is(actualValues.length, expectedValues.length, " Checking length of expected menu");
  for (let i = 0; i < expectedValues.length; i++) {
    is(actualValues[i], expectedValues[i], " Checking menu entry #" + i);
  }
}

function addAddress(address) {
  return new Promise(resolve => {
    formFillChromeScript.sendAsyncMessage("FormAutofillTest:AddAddress", {address});
    formFillChromeScript.addMessageListener("FormAutofillTest:AddressAdded", function onAdded(data) {
      formFillChromeScript.removeMessageListener("FormAutofillTest:AddressAdded", onAdded);

      resolve();
    });
  });
}

function removeAddress(guid) {
  return new Promise(resolve => {
    formFillChromeScript.sendAsyncMessage("FormAutofillTest:RemoveAddress", {guid});
    formFillChromeScript.addMessageListener("FormAutofillTest:AddressRemoved", function onDeleted(data) {
      formFillChromeScript.removeMessageListener("FormAutofillTest:AddressRemoved", onDeleted);

      resolve();
    });
  });
}

function updateAddress(guid, address) {
  return new Promise(resolve => {
    formFillChromeScript.sendAsyncMessage("FormAutofillTest:UpdateAddress", {address, guid});
    formFillChromeScript.addMessageListener("FormAutofillTest:AddressUpdated", function onUpdated(data) {
      formFillChromeScript.removeMessageListener("FormAutofillTest:AddressUpdated", onUpdated);

      resolve();
    });
  });
}

function getAddresses() {
  return new Promise(resolve => {
    formFillChromeScript.sendAsyncMessage("FormAutofillTest:GetAddresses");
    formFillChromeScript.addMessageListener("FormAutofillTest:Addresses", function onUpdated(data) {
      formFillChromeScript.removeMessageListener("FormAutofillTest:Addresses", onUpdated);

      resolve(data);
    });
  });
}

function areAddressesMatching(addressA, addressB) {
  for (let field of VALID_ADDRESS_FIELDS) {
    if (addressA[field] !== addressB[field]) {
      return false;
    }
  }
  return true;
}

function checkAddresses(expectedAddresses) {
  return getAddresses().then((addresses => {
    is(addresses.length, expectedAddresses.length, "Number of address are matching");
    for (let address of addresses) {
      for (let expectedAddress of expectedAddresses) {
        ok(areAddressesMatching(address, expectedAddress), "2 Addresses' content are matching");
      }
    }
  }));
}

function formAutoFillCommonSetup() {
  let chromeURL = SimpleTest.getTestFileURL("formautofill_parent_utils.js");
  formFillChromeScript = SpecialPowers.loadChromeScript(chromeURL);
  formFillChromeScript.addMessageListener("onpopupshown", ({results}) => {
    gLastAutoCompleteResults = results;
    if (gPopupShownListener) {
      gPopupShownListener({results});
    }
  });

  SimpleTest.registerCleanupFunction(() => {
    formFillChromeScript.sendAsyncMessage("cleanup");
    formFillChromeScript.destroy();
  });
}

formAutoFillCommonSetup();
