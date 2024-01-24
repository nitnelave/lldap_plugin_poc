local on_get_user = function (handler, arg)
  handler:get_user(arg)
  return arg
  -- return {name = arg.name}
end

return {
  { event = "on_get_user", priority = 50, impl = on_get_user},
}
