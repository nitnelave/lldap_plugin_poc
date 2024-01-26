-- PoC plugin with an imaginary API.

-- Called on startup.
local initialize_attributes = function(context)
  local schema = context.api.get_schema()
  if schema.user_attributes.attributes.uidnumber = nil then
    local res = context.api:add_user_attribute({
      name = "uidnumber", 
      attribute_type = "Integer",
      is_list = false,
      is_visible = true,
      is_editable = false,
    })
    if res ~= nil then
      -- Error.
      return res
    end
  end
  -- Other PAM attributes, group ID, etc.
end

local on_create_user = function (context, args)
  -- Ensure that the uidNumber for the created user is unique.
  local uids = {}
  local max_uid = 0
  -- Collect all the uidNumbers
  --
  -- Plugins are encouraged not to hold mutable state:
  --   - For HA deployments, not all instances will see the query.
  --   - Lua is not thread safe, concurrent writes will cause race conditions.

  -- You can call API functions from inside a listener.
  -- Arguments can have default values.
  local users, err = context.api:list_users({})

  if err ~= nil then
    -- Error
    return err
  end

  for user_and_group in users do
    local uid = user_and_group.user.attributes.uidnumber
    if uid > max_uid then
      max_uid = uid
    end
    uids[uid] = true
  end

  if args.attributes.uidnumber ~= nil then
    if uids[args.attributes.uidnumber] then
      return nil, "Cannot create user " .. args.user_id .. " with uidNumber " .. args.attributes.uidnumber .. ": uidNumber is already taken"
    end
  else
    -- You can change the arguments to the function.
    args.attributes.uidnumber = max_uid + 1
  end
  -- The returned args will replace the original args.
  return args
end

return {
  name = "pam",
  version = "1.2",
  author = "nitnelave",
  repo = "https://github.com/nitnelave/lldap_plugin_poc/lua/pam.lua",
  init = initialize_attributes,
  listeners = {
    -- Which event you subscribe to, the priority (highest gets called first), and the function to call.
    { event = "on_create_user", priority = 50, impl = on_create_user},
  },
}
