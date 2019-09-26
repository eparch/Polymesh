// PM2 config file
module.exports = {
  apps: [
    {
      name: "pmesh-primary-node",
      script:
        "../../polymesh_substrate/target/release/polymesh-substrate",
      args: "-d /tmp/pmesh-primary-node --pool-limit 100000 --ws-port 9944 --alice --validator --force-authoring",
      env: {
        RUST_BACKTRACE: "1"
      }
    },
    {
      name: "pmesh-peer-node-1",
      script:
        "../../polymesh_substrate/target/release/polymesh-substrate",
      args: "-d /tmp/pmesh-peer-node-1 --ws-port 9945 --bob --validator --force-authoring",
      env: {
        RUST_BACKTRACE: "1"
      }
    },
    {
      name: "pmesh-peer-node-2",
      script:
        "../../polymesh_substrate/target/release/polymesh-substrate",
      args: "-d /tmp/pmesh-peer-node-2 --ws-port 9946 --charlie --validator --force-authoring",
      env: {
        RUST_BACKTRACE: "1"
      }
    },
    {
      name: "stats-collector",
      script: "./index.js",
    }
  ]
};
