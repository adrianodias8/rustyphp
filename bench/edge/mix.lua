-- wrk script for bench/edge/purge-roundtrip.sh: a share AUTH (default 0.1)
-- of the requests carries the session cookie in SESSION, the rest are anonymous.
local cookie = os.getenv("SESSION")
local share = tonumber(os.getenv("AUTH") or "0.1")
request = function()
  if math.random() < share then return wrk.format("GET", "/", { ["Cookie"] = cookie }) end
  return wrk.format("GET", "/")
end
