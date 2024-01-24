-- Gets called when the "get_user" function is called.
local on_get_user = function (api, args)
  -- You can call other functions of the API.
  api:get_user(args)
  -- You can change the arguments to the function.
  args.name = "modified_name"
  -- Or replace them completely
  args = { name = "bob" }
  -- The returned args will replace the original args.
  return args
end

return {
  -- Which event you subscribe to, the priority (highest gets called first), and the function to call.
  { event = "on_get_user", priority = 50, impl = on_get_user},
}
