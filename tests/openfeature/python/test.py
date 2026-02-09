from openfeature import api
from openfeature.contrib.provider.ofrep import OFREPProvider
from openfeature.evaluation_context import EvaluationContext

# configure OFREP provider pointing at local ff serve
api.set_provider(OFREPProvider("http://localhost:8080"))

# create a client
client = api.get_client()

# evaluate FF-feature-y with countryCode=nl context
context = EvaluationContext(attributes={"countryCode": "nl"})
flag_value = client.get_boolean_value("FF-feature-y", False, context)
assert flag_value is True, f"Expected True, got {flag_value}"
print("PASS FF-feature-y(countryCode=nl) == " + str(flag_value))

## timeout check
flag_value = client.get_integer_value("FF-api-timeout", 2000, context)
assert flag_value == 5000, "Integer value doesnt work"
print("PASS FF-api-timeout == " + str(flag_value))

## string check
flag_value = client.get_string_value("FF-log-level", "info", context)
assert flag_value == "debug", "String value doesnt work"
print("PASS FF-log-level == " + str(flag_value))