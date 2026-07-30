// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "./access/EmergencyControl.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/**
 * @title CoreEngine
 * @notice EVM-side custody and execution-intent boundary for Chen Pilot.
 *
 * @dev This contract deliberately does not execute cross-chain transactions,
 * call bridges, or select external liquidity venues. Its EVM responsibility is
 * limited to:
 *
 *  - holding user ERC20 principal;
 *  - recording validated execution intents submitted by authorized clients;
 *  - exposing an emergency principal withdrawal path while paused; and
 *  - emitting versioned records for off-chain execution infrastructure.
 *
 * An authorized client may consume the emitted intent and perform work on an
 * external venue or chain. Completion and settlement of that external work are
 * outside this contract's trust boundary and must not be inferred from these
 * events alone.
 */
contract CoreEngine is EmergencyControl, ReentrancyGuard {
    using SafeERC20 for IERC20;

    /// @notice Version of the event schema emitted by this boundary.
    uint256 public constant EVENT_VERSION = 1;

    /// @notice Accounts permitted to submit execution intents.
    mapping(address => bool) public authorizedClients;

    /// @notice User principal held by this contract, tracked per ERC20 token.
    mapping(address => mapping(address => uint256)) public userPrincipal;

    event Deposited(
        uint256 version,
        address indexed actor,
        address indexed user,
        address indexed token,
        uint256 amount
    );

    event Swapped(
        uint256 version,
        address indexed actor,
        address indexed user,
        address indexed fromToken,
        address toToken,
        uint256 amount
    );

    event Rebalanced(
        uint256 version,
        address indexed actor,
        address indexed user,
        address[] tokens,
        uint256[] amounts
    );

    event EmergencyWithdrawn(
        uint256 version,
        address indexed actor,
        address indexed user,
        address indexed token,
        uint256 amount
    );

    event ClientAuthorized(
        uint256 version,
        address indexed actor,
        address indexed client
    );

    event ClientRevoked(
        uint256 version,
        address indexed actor,
        address indexed client
    );

    modifier onlyOwner() {
        require(hasRole(DEFAULT_ADMIN_ROLE, msg.sender), "Not owner");
        _;
    }

    modifier onlyAuthorizedClient() {
        require(authorizedClients[msg.sender], "Not authorized client");
        _;
    }

    /**
     * @notice Authorize an off-chain or contract-based execution client.
     * @dev Authorization grants permission to submit intents only. It does
     * not grant permission to withdraw user principal or administer roles.
     */
    function grantClient(address client) external onlyOwner {
        require(client != address(0), "Invalid client");
        authorizedClients[client] = true;
        emit ClientAuthorized(EVENT_VERSION, msg.sender, client);
    }

    /**
     * @notice Revoke an execution client's permission to submit intents.
     */
    function revokeClient(address client) external onlyOwner {
        authorizedClients[client] = false;
        emit ClientRevoked(EVENT_VERSION, msg.sender, client);
    }

    /**
     * @notice Deposit ERC20 principal into the EVM boundary.
     * @param token ERC20 token to deposit.
     * @param amount Amount of principal to deposit.
     */
    function deposit(
        address token,
        uint256 amount
    ) external whenNotPaused nonReentrant {
        require(token != address(0), "Invalid token");
        require(amount > 0, "Amount must be greater than zero");

        IERC20(token).safeTransferFrom(msg.sender, address(this), amount);
        userPrincipal[msg.sender][token] += amount;

        emit Deposited(EVENT_VERSION, msg.sender, msg.sender, token, amount);
    }

    /**
     * @notice Submit a local execution intent for an authorized executor.
     *
     * @dev This function intentionally does not move tokens or claim that a
     * swap occurred. The event is an EVM boundary record consumed by external
     * execution infrastructure. The legacy name is retained for ABI
     * compatibility.
     */
    function swap(
        address user,
        address fromToken,
        address toToken,
        uint256 amount
    ) external whenNotPaused nonReentrant onlyAuthorizedClient {
        require(user != address(0), "Invalid user");
        require(fromToken != address(0), "Invalid source token");
        require(toToken != address(0), "Invalid destination token");
        require(fromToken != toToken, "Tokens must differ");
        require(amount > 0, "Amount must be greater than zero");

        emit Swapped(
            EVENT_VERSION,
            msg.sender,
            user,
            fromToken,
            toToken,
            amount
        );
    }

    /**
     * @notice Submit a local portfolio rebalance intent.
     *
     * @dev No asset movement occurs here. Execution and settlement belong to
     * the authorized client and the external venue it integrates with.
     * The legacy name is retained for ABI compatibility.
     */
    function rebalance(
        address user,
        address[] calldata tokens,
        uint256[] calldata amounts
    ) external whenNotPaused nonReentrant onlyAuthorizedClient {
        require(user != address(0), "Invalid user");
        require(tokens.length == amounts.length, "Invalid input lengths");
        require(tokens.length > 0, "Empty rebalance");

        for (uint256 i = 0; i < tokens.length; i++) {
            require(tokens[i] != address(0), "Invalid token");
            require(amounts[i] > 0, "Amount must be greater than zero");
        }

        emit Rebalanced(EVENT_VERSION, msg.sender, user, tokens, amounts);
    }

    /**
     * @notice Withdraw all recorded principal for a token during an emergency.
     * @dev Only the caller's recorded principal is returned. No yield or
     * unaccounted balance is distributed by this function.
     */
    function emergencyWithdraw(
        address token
    ) external whenPaused nonReentrant {
        require(token != address(0), "Invalid token");

        uint256 principal = userPrincipal[msg.sender][token];
        require(principal > 0, "No principal to withdraw");

        userPrincipal[msg.sender][token] = 0;
        IERC20(token).safeTransfer(msg.sender, principal);

        emit EmergencyWithdrawn(
            EVENT_VERSION,
            msg.sender,
            msg.sender,
            token,
            principal
        );
    }
}
