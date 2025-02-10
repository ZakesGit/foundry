# FROM: ./my_token

touch src/MyToken.sol;
# FROM: ./my_token

forge create --rpc-url https://rpc.berachain.com/ --private-key <rS1ad-YbuD6vKG77ZwTmsaNGC19pmyyN> src/MyToken.sol:MyToken --legacy;
