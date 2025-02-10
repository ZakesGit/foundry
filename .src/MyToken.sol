// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol"; 

contract MyToken is ERC20 {
    uint256 public constant INITIAL_SUPPLY = 1_000_000 * 1 ether;

    constructor() ERC20("MyToken", "MT"){
        _mint(msg.sender, INITIAL_SUPPLY);
    }
}
// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

contract MyToken is ERC20 {
    uint256 public constant INITIAL_SUPPLY = 1_000_000 * 1 ether;
    uint256 public constant BURN_PERCENTAGE = 1; // 1%
    address public constant BURN_ADDRESS = 0xDeaDbeefdEAdbeefdEadbEEFdeadbeEFdEaDbeeF; 

    constructor() ERC20("MyToken", "MT"){
        _mint(msg.sender, INITIAL_SUPPLY);
    }

    function _update(address sender, address recipient, uint256 amount) internal override { 
        uint256 burnAmount = (amount * BURN_PERCENTAGE) / 100;
        super._update(sender, recipient, amount - burnAmount);
        super._update(sender, BURN_ADDRESS, burnAmount);
    }
}    
