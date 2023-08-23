// SPDX-License-Identifier: MIT
pragma solidity =0.8.19;

import "@openzeppelin/contracts/utils/structs/EnumerableSet.sol";

contract AccessGrants {
    using EnumerableSet for EnumerableSet.Bytes32Set;

    struct Grant {
      address owner;
      address grantee;
      string dataId;
      uint256 lockedUntil;
    }

    mapping(bytes32 => Grant) private _grantsById;

    mapping(address => EnumerableSet.Bytes32Set) private _grantIdsByOwner;
    mapping(address => EnumerableSet.Bytes32Set) private _grantIdsByGrantee;
    mapping(string => EnumerableSet.Bytes32Set) private _grantIdsByDataId;

    bytes32 private constant _WILDCARD_DATA_ID = keccak256(abi.encodePacked("0"));

    constructor() {}

    function insertGrant(
      address grantee,
      string memory dataId,
      uint256 lockedUntil
    ) external {
        Grant memory grant = Grant({
            owner: msg.sender,
            grantee: grantee,
            dataId: dataId,
            lockedUntil: lockedUntil
        });

        bytes32 grantId = _deriveGrantId(grant);

        require(_grantsById[grantId].owner == address(0), "Grant already exists");

        _grantsById[grantId] = grant;
        _grantIdsByOwner[grant.owner].add(grantId);
        _grantIdsByGrantee[grant.grantee].add(grantId);
        _grantIdsByDataId[grant.dataId].add(grantId);
    }

    function deleteGrant(
      address grantee,
      string memory dataId,
      uint256 lockedUntil
    ) external {
        Grant[] memory grants = grantsBy(msg.sender, grantee, dataId);

        require(grants.length > 0, "No grants for sender");

        for (uint256 i = 0; i < grants.length; i++) {
          Grant memory grant = grants[i];

          if (lockedUntil == 0 || grants[i].lockedUntil == lockedUntil) {
            require(grant.lockedUntil < block.timestamp, "Grant is timelocked");

            bytes32 grantId = _deriveGrantId(grant);

            delete _grantsById[grantId];
            _grantIdsByOwner[grant.owner].remove(grantId);
            _grantIdsByGrantee[grant.grantee].remove(grantId);
            _grantIdsByDataId[grant.dataId].remove(grantId);
          }
        }
    }

    function grantsFor(
      address grantee,
      string memory dataId
    ) external view returns (Grant[] memory) {
        return grantsBy(address(0), grantee, dataId);
    }

    function grantsBy(
      address owner,
      address grantee,
      string memory dataId
    ) public view returns (Grant[] memory) {
        bytes32[] memory candidateGrantIds;
        uint256 candidateGrantCount;

        if (owner != address(0)) {
          candidateGrantIds = _grantIdsByOwner[owner].values();
          candidateGrantCount = _grantIdsByOwner[owner].length();
        } else if (grantee != address(0)) {
          candidateGrantIds = _grantIdsByGrantee[grantee].values();
          candidateGrantCount = _grantIdsByGrantee[grantee].length();
        } else {
          revert("Neither owner nor grantee provided");
        }

        uint256 returnCount = 0;
        bool[] memory keepList = new bool[](candidateGrantCount);

        for (uint256 i = 0; i < candidateGrantCount; i++) {
            bytes32 candidateGrantId = candidateGrantIds[i];
            bool returnCandidate = false;

            returnCandidate =
              grantee == address(0)
              || _grantIdsByGrantee[grantee].contains(candidateGrantId);

            returnCandidate = returnCandidate && (
              _isWildcardDataId(dataId)
              || _grantIdsByDataId[dataId].contains(candidateGrantId)
            );

            if (returnCandidate) {
                returnCount++;
                keepList[i] = true;
            }
        }

        Grant[] memory grants = new Grant[](returnCount);
        uint256 returnIndex = 0;

        for (uint256 i = 0; i < candidateGrantCount; i++) {
            if (keepList[i]) {
                grants[returnIndex] = _grantsById[candidateGrantIds[i]];
                returnIndex++;
            }
        }

        return grants;
    }

    function _deriveGrantId(
      Grant memory grant
    ) private pure returns (bytes32) {
        return keccak256(abi.encodePacked(
          grant.owner,
          grant.grantee,
          grant.dataId,
          grant.lockedUntil
        ));
    }

    function _isWildcardDataId(
        string memory dataId
    ) private pure returns (bool) {
      return keccak256(abi.encodePacked((dataId))) == _WILDCARD_DATA_ID;
    }
}
