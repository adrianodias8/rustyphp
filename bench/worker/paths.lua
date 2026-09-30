-- wrk request script: cycle the five paths of bench/symfony-boot.php's
-- handle_requests section, so the load matches the in-process benchmark.
local paths = { "/", "/user/42/alice", "/api/widgets?page=2&sort=name", "/user/7", "/api/orders?x[]=1&x[]=2" }
local i = 0
request = function()
  i = i + 1
  return wrk.format("GET", paths[(i % #paths) + 1])
end
